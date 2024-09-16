use std::collections::HashMap;
use crate::opcodes;
//use bitflags::bitflags;

// bitflags!{
//     pub struct CpuFlags:u8{
//         const CARRY            =0b00000001;
//         const ZERO             =0b00000010;
//         const INTERRUPT_DISABLE=0b00000100;
//         const DECIMAL_MODE     =0b00001000;
//         const BREAK            =0b00010000;
//         const BREAK2           =0b00100000;//未使用
//         const OVERLOW          =0b01000000;
//         const NEGARIVE         =0b10000000;
//     }
// }

pub struct CPU{
    pub register_a:u8,//Acumulator
    pub register_x:u8,
    pub register_y:u8,
    pub status: u8,/*flag
    |Negative|oVerflow| |Break command|
    Decimal mode flag|Interpret disable|Zero flag|Carry flag
    */
    pub program_counter:u16,
    memory:[u8;0xFFFF],

}


#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode{
    Immediate,
    ZeroPage,//現在のPCがさす8byteの情報をを16byteのアドレスとみなしてメモリにアクセス→register_aに格納
    ZeroPage_X,//現在のPCがさす8byteの情報+register_xを16byteのアドレスとみなす
    ZeroPage_Y,
    Absolute,//16byteのフルアクセス
    Absolute_X,
    Absolute_Y,
    Indirect_X,
    Indirect_Y,
    NoneAddressing,
}
impl CPU{
    pub fn new()->Self{
        CPU { 
            register_a: 0, 
            register_x:0,
            register_y:0,
            status: 0,//CpuFlags::from_bits_truncate(0b100100), //0b0000_0000
            program_counter: 0, 
            memory:[0;0xFFFF],
        }
    } 

    fn get_operand_address(&self,mode:&AddressingMode)->u16{
        match mode {
            AddressingMode::Immediate=>self.program_counter,
            AddressingMode::ZeroPage=>self.mem_read(self.program_counter) as u16,
            AddressingMode::Absolute=>self.mem_read_u16(self.program_counter),
            AddressingMode::ZeroPage_X=>{
                let pos =self.mem_read(self.program_counter);
                let addr=pos.wrapping_add(self.register_x) as u16;
                addr
            }
            AddressingMode::ZeroPage_Y=>{
                let pos =self.mem_read(self.program_counter);
                let addr =pos.wrapping_add(self.register_y) as u16;
                addr
            }
            AddressingMode::Absolute_X=>{
                let base =self.mem_read_u16(self.program_counter);
                let addr =base.wrapping_add(self.register_x as u16);
                addr
            }
            AddressingMode::Absolute_Y=>{
                let base =self.mem_read_u16(self.program_counter);
                let addr =base.wrapping_add(self.register_y as u16);
                addr
            }
            AddressingMode::Indirect_X=>{
                let base =self.mem_read(self.program_counter);

                let ptr:u8=(base as u8).wrapping_add(self.register_x);
                let lo =self.mem_read(ptr as u16);
                let hi =self.mem_read(ptr.wrapping_add(1) as u16);
                (hi as u16)<<8|(lo as u16)
            }
            AddressingMode::Indirect_Y=>{
                let base=self.mem_read(self.program_counter);

                let lo =self.mem_read(base as u16);
                let hi=self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base=(hi as u16)<<8|(lo as u16);
                let deref=deref_base.wrapping_add(self.register_y as u16);
                deref
            }
            AddressingMode::NoneAddressing=>{
                panic!("mode {:?} id not supported",mode);
            }
        }
    }

    fn adc(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.add_to_register_a(value);
    }

    fn lda(&mut self, mode: &AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.register_a=value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ldx(&mut self, value:u8){
        self.register_x=value;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn sta(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        self.mem_write(addr,self.register_a);
    }

    fn tax(&mut self){
        self.register_x=self.register_a;
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn inx(&mut self){
        self.register_x=self.register_x.wrapping_add(1);
        /*
        if self.register_x==0xff{
            self.register_x=0x00;
        }else{
            self.register_x+=1;
        }
        */
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn update_zero_and_negative_flags(&mut self, result:u8){
        if result==0{//Zero flag
            self.status=self.status|0b0000_0010;
        }else{
            self.status=self.status&0b1111_1101;
        }

        if result&0b1000_0000!=0{//negative flag
            self.status=self.status|0b1000_0000;
        }else{
            self.status=self.status&0b0111_1111;
        }
    }

    fn update_carry_flag(&mut self,result:u16){
        let carry =result>0xff;

        if carry {
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }
    }

    fn update_overflow_flag(&mut self,data:u8,value:u8,result:u8){
        if (data^/*XOR*/result)&(value^result)&0b1000_0000!=0{
            self.status=self.status|0b0100_0000;
        }else{
            self.status=self.status&0b1011_1111;
        }
    }


    fn add_to_register_a(&mut self,data:u8){
        let sum=self.register_a as u16
                    +data as u16
                    +(
                        if self.status&0000_0001!=0{
                            1
                        }else{
                            0
                        }
                    )as u16;

        self.update_overflow_flag(data,self.register_a,sum as u8);
        self.update_carry_flag(sum);
        self.register_a=sum as u8;
        self.update_zero_and_negative_flags(self.register_a);

    }

    /*オペランドなどが8バイトなのに対して、アドレスは16バイト */
    fn mem_read(&self,addr:u16)->u8{
        self.memory[addr as usize]
    }

    fn mem_read_u16(&self, pos:u16)->u16{
        let lo=self.mem_read(pos) as u16;
        let hi =self.mem_read(pos+1) as u16;
        (hi<<8)|(lo as u16)//<<は左シフト演算子
    }

    fn mem_write(&mut self, addr:u16,data:u8){
        self.memory[addr as usize]=data;
    }

    fn mem_write_u16(&mut self, pos:u16,data:u16){
        let hi=(data>>8) as u8;
        let lo=(data & 0b00000000_11111111) as u8;
        self.mem_write(pos,lo);
        self.mem_write(pos+1,hi);
    }


    pub fn load_and_run(&mut self,program:Vec<u8>){
        self.load(program);
        self.reset();
        self.run();
    }

    pub fn load(&mut self,program:Vec<u8>){
        self.memory[0x8000..(0x8000+program.len())].copy_from_slice(&program[..]);
        //memory[0c8000..(ox8000+program.len())]にprogram[..]の内容を格納する。
        self.mem_write_u16(0xFFFC,0x8000);
    }

    pub fn reset(&mut self){
        self.register_a=0;
        self.register_x=0;
        self.status=0;

        self.program_counter=self.mem_read_u16(0xFFFC);
    }


    pub fn run(&mut self){
        let ref opcodes:HashMap<u8,&'static opcodes::OpCode>=*opcodes::OPCODES_MAP;
        loop{
            let code =self.mem_read(self.program_counter);
            self.program_counter+=1;

            let opcode=opcodes.get(&code).expect(&format!("OpCode {:?} is not recognized",code));
            match code{
                0xA2=>{
                    let param=self.mem_read(self.program_counter);
                    self.program_counter+=1;

                    self.ldx(param);
                }

                //LDA
                0xA9|0xA5|0xB5|0xAD|0xBD|0xB9|0xA1|0xB1=>{
                    self.lda(&opcode.mode);
                    self.program_counter+=(opcode.len-1) as u16;
                }

                //STA
                0x85|0x95|0x8d|0x9d|0x99|0x81|0x91=>{
                    self.sta(&opcode.mode);
                    self.program_counter+=(opcode.len-1) as u16;
                }

                //ADC
                0x69|0x65|0x75|0x6D|0x7D|0x79|0x61|0x71=>{
                    self.adc(&opcode.mode);
                    self.program_counter+=(opcode.len-1) as u16;
                }

                //TAX
                0xAA=>{
                    self.tax();
                }

                0xE8=>{
                    self.inx();
                }

                0x00=>{
                    return;
                }
                _=>todo!()
            }
        }
    }


    pub fn interpret(&mut self, program:Vec<u8>){
        self.load_and_run(program);
    }
}

//test----------------------------------------------------------------------------
#[cfg(test)]
mod test{
    use super::*;

    #[test]
    fn test_0xa9_lda_immediate_load_data(){
        let mut cpu=CPU::new();
        cpu.interpret(vec![0xa9,0x05,0x00]);//十六進数表記の話なので’a’と’A’に違いはない
        assert_eq!(cpu.register_a, 0x05);
        assert!(cpu.status&0b0000_0010==0b00);
        assert!(cpu.status&0b1000_0000==0);
    }

    #[test]
    fn test_0xa9_lda_zero_flag(){
        let mut cpu=CPU::new();
        cpu.interpret(vec![0xa9,0x00,0x00]);
        assert!(cpu.status&0b0000_0010==0b10);
    }
    
    #[test]
    fn test_0xa9_lda_negative_flag() {
        let mut cpu = CPU::new();
        cpu.interpret(vec![0xa9, 0xff, 0x00]);
        assert!(cpu.status & 0b1000_0000 == 0b1000_0000);

    }


    #[test]
    fn test_0xaa_tax(){
        let mut cpu=CPU::new();
        cpu.interpret(vec![0xa9,10,0xaa,0x00]);
        assert_eq!(cpu.register_x,10);
    }

    #[test]
    fn test_5_ops_working_together(){
        let mut cpu =CPU::new();
        cpu.interpret(vec![0xa9,0xc0,0xaa,0xe8,0x00]);

        assert_eq!(cpu.register_x,0xc1);
    }
    #[test]
    fn test_inx_overflow(){
        let mut cpu=CPU::new();
        cpu.interpret(vec![0xa2,0xff,0xe8,0xe8,0x00]);

        assert_eq!(cpu.register_x,1);
    }

    #[test]//LDA
    fn test_lda_from_memory_zero_page(){
        let address:u8=0x10;
        let mut cpu=CPU::new();
        cpu.mem_write(address as u16,0x55);
        cpu.load_and_run(vec![0xa5,address,0x00]);

        assert_eq!(cpu.register_a,0x55);
    }

    #[test]
    fn test_lda_from_memory_zero_page_x(){
        let mut cpu=CPU::new();
        cpu.load(vec![0xB5,0xA0,0x00]);
        cpu.reset();
        cpu.register_x=0x01;
        cpu.mem_write(0xA1,0x44);
        cpu.run();

        assert_eq!(cpu.register_a,0x44);
    }

    #[test]
    fn test_lda_from_memory_absolute(){
        let mut cpu=CPU::new();
        cpu.load(vec![0xad,0x10,0x20,0x00]);
        cpu.reset();
        cpu.mem_write(0x2010,0x77);
        cpu.run();

        assert_eq!(cpu.register_a,0x77);
    }

    #[test]
    fn test_lda_from_memory_absolute_x (){
        let mut cpu=CPU::new();
        cpu.load(vec![0xbd,0x10,0x20,0x00]);
        cpu.reset();
        cpu.register_x=0x05;
        cpu.mem_write(0x2015,0x66);
        cpu.run();

        assert_eq!(cpu.register_a,0x66);
    }

    #[test]
    fn test_lda_from_memory_absolute_y (){
        let mut cpu=CPU::new();
        cpu.load(vec![0xb9,0x10,0x30,0x00]);
        cpu.reset();
        cpu.register_y=0x05;
        cpu.mem_write(0x3015,0x88);
        cpu.run();

        assert_eq!(cpu.register_a,0x88);
    }

    #[test]
    fn test_lda_from_memory_indirect_x (){
        let mut cpu=CPU::new();
        cpu.load(vec![0xA1,0x10,0x00]);
        cpu.reset();
        cpu.register_x=0x03;

        cpu.mem_write(0x10+0x03,0x20);
        cpu.mem_write(0x10+0x03+0x01,0x30);
        cpu.mem_write(0x3020,0x33);
        cpu.run();

        assert_eq!(cpu.register_a,0x33);
    }

    #[test]
    fn test_lda_from_memory_indirect_y (){
        let mut cpu=CPU::new();
        cpu.load(vec![0xB1,0x10,0x00]);
        cpu.reset();
        cpu.mem_write(0x10,0x20);
        cpu.mem_write(0x10+0x01,0x30);
        cpu.register_y=0x05;
        cpu.mem_write((0x30 << 8)+0x20+0x05,0x07);
        cpu.run();

        assert_eq!(cpu.register_a,0x07);
    }

    //STA
    #[test]
    fn test_sta_from_memory() {
        let mut cpu =CPU::new();
        cpu.load_and_run(vec![0xA9,0xBA,0x85, 0x10, 0x00]); 
        assert_eq!(cpu.mem_read(0x10), 0xBA);
    }   
    //STAのほかのテストも作らないと
    
    // ADC
    #[test]
    fn test_adc_no_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x20;
        cpu.run();

        assert_eq!(cpu.register_a, 0x30);
        assert_eq!(cpu.status,0b0000_0000);
    }

    #[test]
    fn test_adc_has_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x20;
        cpu.status=0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x31);
        assert_eq!(cpu.status,0b0000_0000);
    }

    #[test]
    fn test_adc_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x01, 0x00]);
        cpu.reset();
        cpu.register_a=0xFF;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.status,0b0000_0011);
    }

    #[test]
    fn test_adc_occur_overflow_plus() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x7F;
        cpu.run();
        assert_eq!(cpu.register_a, 0x8F);
        assert_eq!(cpu.status,0b1100_0000);//carryflagいらないの？
    }

    #[test]
    fn test_adc_occur_overflow_plus_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x6F, 0x00]);
        cpu.reset();
        cpu.register_a=0x10;
        cpu.status=0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x80);
        assert_eq!(cpu.status,0b1100_0000);
    }

    #[test]
    fn test_adc_occur_overflow_minus() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x81, 0x00]);
        cpu.reset();
        cpu.register_a=0x81;
        cpu.run();
        assert_eq!(cpu.register_a, 0x02);
        assert_eq!(cpu.status,0b0100_0001);
    }

    #[test]
    fn test_adc_occur_overflow_minus_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x80, 0x00]);
        cpu.reset();
        cpu.register_a=0x80;
        cpu.status=0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status,0b0100_0001);
    }

    #[test]
    fn test_adc_no_overflow() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x7F, 0x00]);
        cpu.reset();
        cpu.register_a=0x82;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status,0b0000_0001);
    }

}

