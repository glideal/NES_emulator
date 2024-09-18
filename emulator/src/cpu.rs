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

const STACK:u16=0x0100;
const STACK_RESET:u8=0xFD;//0xFFではなく0xFDとするのは安全性を考慮した結果の慣習
pub struct CPU{
    pub register_a:u8,//Acumulator
    pub register_x:u8,
    pub register_y:u8,
    pub status: u8,
    ///  7 6 5 4 3 2 1 0
    ///  N V _ B D I Z C
    ///  | |   | | | | +--- Carry Flag
    ///  | |   | | | +----- Zero Flag
    ///  | |   | | +------- Interrupt Disable
    ///  | |   | +--------- Decimal Mode (not used on NES)
    ///  | |   +----------- Break Command
    ///  | +--------------- Overflow Flag
    ///  +----------------- Negative Flag
    stack_pointer:u8,
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
            stack_pointer:STACK_RESET,
            status: 0b0010_0000,//CpuFlags::from_bits_truncate(0b100100), //0b0000_0000
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

    fn sbc(&mut self,mode:&AddressingMode){
        let addr =self.get_operand_address(mode);
        let value=self.mem_read(addr);

        let mut data=(0b1111_1111)-value+1;//=-(value)//補数表現
        data=data.wrapping_sub(1);//キャリーフラグによる減算のため
        self.add_to_register_a(data/*(value as i8).wrapping_neg().wrapping_sub(1)as u8*/);
    }

    fn adc(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.add_to_register_a(value);
    }

    //logical calculation
    fn and(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.register_a=self.register_a&value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn eor(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.register_a=self.register_a^value;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ora(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        self.register_a=self.register_a|value;
        self.update_zero_and_negative_flags(self.register_a);
    }
    

    //shift calculation
    fn asl_accumulator(&mut self){//ArithmeticもLogicalも変わらない
        if self.register_a&0b1000_0000!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        self.register_a=self.register_a<<1;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn asl(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut data=self.mem_read(addr);

        if data&0b1000_0000!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        data=data<<1;
        self.mem_write(addr,data);
        self.update_zero_and_negative_flags(data);
    }


    fn lsr_accumulator(&mut self){
        if self.register_a&0b0000_0001!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        self.register_a=self.register_a>>1;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn lsr(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut data=self.mem_read(addr);

        if data&0b0000_0001!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        data=data>>1;
        self.mem_write(addr,data);
        self.update_zero_and_negative_flags(data);
    }

    fn rol_accumulator(&mut self){
        let mut tmp=self.register_a;
        if self.status&0b0000_0001!=0{//キャラ―フラグが１だったら
            tmp=(tmp<<1)|0b0000_0001;
        }else{
            tmp=(tmp<<1)&0b1111_1110;
        }        

        if self.register_a&0b1000_0000!=0{//7ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        self.register_a=tmp;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn rol(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut data=self.mem_read(addr);

        let mut tmp=data;
        if self.status&0b0000_0001!=0{//キャラ―フラグが１だったら
            tmp=(tmp<<1)|0b0000_0001;
        }else{
            tmp=(tmp<<1)&0b1111_1110;
        }        

        if data&0b1000_0000!=0{//7ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        data=tmp;
        self.mem_write(addr,data);
        self.update_zero_and_negative_flags(self.mem_read(addr));
    }

    fn ror_accumulator(&mut self){
        let mut tmp=self.register_a;
        if self.status&0b0000_0001!=0{//キャラ―フラグが１だったら
            tmp=(tmp>>1)|0b1000_0000;
        }else{
            tmp=(tmp>>1)&0b0111_1111;
        }        

        if self.register_a&0b0000_0001!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        self.register_a=tmp;
        self.update_zero_and_negative_flags(self.register_a);
    }

    fn ror(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut data=self.mem_read(addr);

        let mut tmp=data;
        if self.status&0b0000_0001!=0{//キャラ―フラグが１だったら
            tmp=(tmp>>1)|0b1000_0000;
        }else{
            tmp=(tmp>>1)&0b0111_1111;
        }        

        if data&0b0000_0001!=0{//0ビット目が1だったら
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }        
        data=tmp;
        self.mem_write(addr,data);
        self.update_zero_and_negative_flags(self.mem_read(addr));
    }

    fn inc(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut value=self.mem_read(addr);
        
        value=value.wrapping_add(1);
        self.mem_write(addr,value);
        self.update_zero_and_negative_flags(value);
    }
    
    fn inx(&mut self){
        self.register_x=self.register_x.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_x);
    }


    fn iny(&mut self){
        self.register_y=self.register_y.wrapping_add(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn dec(&mut self,mode:&AddressingMode){
        let addr=self.get_operand_address(mode);
        let mut value=self.mem_read(addr);
        
        value=value.wrapping_sub(1);
        self.mem_write(addr,value);
        self.update_zero_and_negative_flags(value);
    }

    fn dex(&mut self){
        self.register_x=self.register_x.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_x);
    }

    fn dey(&mut self){
        self.register_y=self.register_y.wrapping_sub(1);
        self.update_zero_and_negative_flags(self.register_y);
    }

    fn compare(&mut self,mode:&AddressingMode,target:u8){
        let addr=self.get_operand_address(mode);
        let value=self.mem_read(addr);

        let tmp=target.wrapping_sub(value);
        self.update_zero_and_negative_flags(tmp);
        if target>=value {
            self.status=self.status|0b0000_0001;
        }else{
            self.status=self.status&0b1111_1110;
        }
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
                        if self.status&0000_0001!=0{//キャリーフラグがセットされてたら
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

    fn push(&mut self,value:u8){
        self.stack_pointer=self.stack_pointer.wrapping_sub(1);
        self.mem_write(STACK|(self.stack_pointer as u16),value);
    }

    fn push_u16(&mut self,data:u16){
        let hi=(data>>8) as u8;
        let lo=data as u8;
        self.push(hi);
        self.push(lo);
    }

    fn pop(&mut self)->u8{
        let data=self.mem_read(STACK|(self.stack_pointer as u16));
        self.stack_pointer=self.stack_pointer.wrapping_add(1);
        data
    }

    fn pop_u16(&mut self)->u16{
        let lo=self.pop();
        let hi=self.pop();
        (hi as u16)<<8|(lo as u16)
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
        self.register_y=0;
        self.status=0b0010_0000;
        self.stack_pointer=STACK_RESET;

        self.program_counter=self.mem_read_u16(0xFFFC);
    }


    pub fn run(&mut self){
        let ref opcodes:HashMap<u8,&'static opcodes::OpCode>=*opcodes::OPCODES_MAP;
        loop{
            let code =self.mem_read(self.program_counter);
            self.program_counter+=1;
            let program_counter_state=self.program_counter;

            let opcode=opcodes.get(&code).expect(&format!("OpCode {:?} is not recognized",code));
            match code{
                //BREAK
                0x00=>{
                    return;
                }

                //LDX
                0xA2=>{
                    let param=self.mem_read(self.program_counter);
                    self.ldx(param);
                }

                //LDA
                0xA9|0xA5|0xB5|0xAD|0xBD|0xB9|0xA1|0xB1=>{
                    self.lda(&opcode.mode);
                }

                //STA
                0x85|0x95|0x8d|0x9d|0x99|0x81|0x91=>{
                    self.sta(&opcode.mode);
                }

                //ADC
                0x69|0x65|0x75|0x6D|0x7D|0x79|0x61|0x71=>{
                    self.adc(&opcode.mode);
                }

                //SBC
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => {
                    self.sbc(&opcode.mode);
                }

                //LOGICAL
                //AND
                0x29|0x25|0x35|0x2D|0x3D|0x39|0x21|0x31=>{
                    self.and(&opcode.mode);
                }

                // EOR
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => {
                    self.eor(&opcode.mode);
                }

                // ORA
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => {
                    self.ora(&opcode.mode);
                }


                //SHIFT
                // ASL 
                0x0a => self.asl_accumulator(),
                0x06 | 0x16 | 0x0e | 0x1e => {
                    self.asl(&opcode.mode);
                }

                // LSR 
                0x4a => self.lsr_accumulator(),
                0x46 | 0x56 | 0x4e | 0x5e => {
                    self.lsr(&opcode.mode);
                }
                
                // ROL 
                0x2a => self.rol_accumulator(),
                0x26 | 0x36 | 0x2e | 0x3e => {
                    self.rol(&opcode.mode);
                }

                // ROR 
                0x6a => self.ror_accumulator(),
                0x66 | 0x76 | 0x6e | 0x7e => {
                    self.ror(&opcode.mode);
                }
                
                // INC
                0xe6 | 0xf6 | 0xee | 0xfe => {
                    self.inc(&opcode.mode);
                }

                //INX
                0xE8=>self.inx(),

                // INY
                0xc8 => self.iny(),

                // DEC
                0xc6 | 0xd6 | 0xce | 0xde => {
                    self.dec(&opcode.mode);
                }

                // DEX
                0xca =>self.dex(),
                

                // DEY
                0x88 =>self.dey(),

                // CMP
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => {
                    self.compare(&opcode.mode, self.register_a);
                }

                // CPY
                0xc0 | 0xc4 | 0xcc => {
                    self.compare(&opcode.mode, self.register_y);
                }

                // CPX
                0xe0 | 0xe4 | 0xec => {
                    self.compare(&opcode.mode, self.register_x);
                }

                //BRANCHING
                //JMP
                0x4c=>{
                    let addr=self.mem_read_u16(self.program_counter);
                    self.program_counter=addr;
                    // continue;
                }

                0x6c=>{
                    //0x6cではprogram_counterがさすメモリの値をアドレスとみなし、そのアドレスがさすメモリの値をまたアドレスとみなしてそこへjumpする。
                    let addr=self.mem_read_u16(self.program_counter);
                    let indirect_ref:u16;
                    if (addr&0x00FF)==0x00FF{//0x6cのバグを表現
                        let lo=self.mem_read(addr);
                        let hi=self.mem_read(addr&0xFF00);
                        indirect_ref=(hi as u16)<<8|(lo as u16);
                    }else{
                        indirect_ref=self.mem_read_u16(addr);
                    }
                    self.program_counter=indirect_ref;
                    // continue;
                }

                //JSR//stack系を作った後で
                0x20=>{
                    self.push_u16(self.program_counter+2-1);//RTSで+1するから＜－これは仕様
                    let tmp=self.mem_read_u16(self.program_counter);
                    self.program_counter=tmp;
                    // continue;
                } 
                //RTS
                0x60=>{
                    self.program_counter=self.pop_u16()+1;
                    // continue;
                }

                //TAX
                0xAA=>{
                    self.tax();
                }

                //STACK
                //PHA//PusH register_A
                0x48=>self.push(self.register_a),
                //PLA//PuLl register_A
                0x68=>{
                    self.register_a=self.pop();
                    self.update_zero_and_negative_flags(self.register_a);
                }
                //PHP
                0x08=>self.push(self.status|0b0001_0000),
                //PLP
                0x28=>self.status=self.pop()&0b1110_1111,



                _ =>todo!(),


            }
            if program_counter_state==self.program_counter{
                self.program_counter+=(opcode.len-1) as u16;
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
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_adc_has_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x20;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x31);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_adc_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x01, 0x00]);
        cpu.reset();
        cpu.register_a=0xFF;
        cpu.run();
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.status,0b0010_0011);
    }

    #[test]
    fn test_adc_occur_overflow_plus() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x7F;
        cpu.run();
        assert_eq!(cpu.register_a, 0x8F);
        assert_eq!(cpu.status,0b1110_0000);//carryflagいらないの？
    }

    #[test]
    fn test_adc_occur_overflow_plus_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x6F, 0x00]);
        cpu.reset();
        cpu.register_a=0x10;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x80);
        assert_eq!(cpu.status,0b1110_0000);
    }

    #[test]
    fn test_adc_occur_overflow_minus() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x81, 0x00]);
        cpu.reset();
        cpu.register_a=0x81;
        cpu.run();
        assert_eq!(cpu.register_a, 0x02);
        assert_eq!(cpu.status,0b0110_0001);
    }

    #[test]
    fn test_adc_occur_overflow_minus_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x80, 0x00]);
        cpu.reset();
        cpu.register_a=0x80;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status,0b0110_0001);
    }

    #[test]
    fn test_adc_no_overflow() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x69, 0x7F, 0x00]);
        cpu.reset();
        cpu.register_a=0x82;
        cpu.run();
        assert_eq!(cpu.register_a, 0x01);
        assert_eq!(cpu.status,0b0010_0001);
    }
    
    // SBC
    #[test]
    fn test_sbc_no_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x20;
        cpu.run();
        assert_eq!(cpu.register_a, 0x0F);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_sbc_has_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x10, 0x00]);
        cpu.reset();
        cpu.register_a=0x20;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0x10);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_sbc_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x02, 0x00]);
        cpu.reset();
        cpu.register_a=0x01;
        cpu.run();
        assert_eq!(cpu.register_a, 0xFE);
        assert_eq!(cpu.status,0b1010_0000);
    }

    #[test]
    fn test_sbc_occur_overflow() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x81, 0x00]);
        cpu.reset();
        cpu.register_a=0x7F;
        cpu.run();
        assert_eq!(cpu.register_a, 0xFD);
        assert_eq!(cpu.status,0b1110_0000);
    }

    #[test]
    fn test_sbc_occur_overflow_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x81, 0x00]);
        cpu.reset();
        cpu.register_a=0x7F;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0xFE);
        assert_eq!(cpu.status,0b1110_0000);
    }

    #[test]
    fn test_sbc_no_overflow() {
        let mut cpu=CPU::new();
        cpu.load(vec![0xe9, 0x7F, 0x00]);
        cpu.reset();
        cpu.register_a=0x7E;
        cpu.status=cpu.status|0b0000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0xFF);
        assert_eq!(cpu.status,0b1010_0000);
    }

    //LOGICAL
    // AND
    #[test]
    fn test_and() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x29, 0b0000_1100, 0x00]);
        cpu.reset();
        cpu.register_a=0b0000_1010;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_1000);
        assert_eq!(cpu.status,0b0010_0000);
    }

    // EOR
    #[test]
    fn test_eor() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x49, 0b0000_1100, 0x00]);
        cpu.reset();
        cpu.register_a=0b0000_1010;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0110);
        assert_eq!(cpu.status,0b0010_0000);

    }

    // ORA
    #[test]
    fn test_ora() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x09, 0b0000_1100, 0x00]);
        cpu.reset();
        cpu.register_a=0b0000_1010;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_1110);
        assert_eq!(cpu.status,0b0010_0000);
    }


    //SHIFT
    // ASL
    #[test]
    fn test_asl_a() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x0a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0110);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_asl_zero_page() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x06, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0110);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_asl_a_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x0a,0x00]);
        cpu.reset();
        cpu.register_a=0b1000_0001;
        cpu.run();
        assert_eq!(cpu.register_a,0b0000_0010);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_asl_zero_page_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x06, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b1000_0001);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0010);
        assert_eq!(cpu.status,0b0010_0001);
    }


    // LSR
    #[test]
    fn test_lsr_a() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x4a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0010;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0001);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_lsr_zero_page() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x46, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0010);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0x01);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_lsr_zero_page_zero_flag() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x46, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0001);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0x00);
        assert_eq!(cpu.status,0b0010_0011);
    }

    #[test]
    fn test_lsr_a_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x4a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.run();
        assert_eq!(cpu.register_a,0x01);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_lsr_zero_page_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x46, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0x01);
        assert_eq!(cpu.status,0b0010_0001);
    }


    // ROL
    #[test]
    fn test_rol_a() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x2a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0110);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_rol_zero_page() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x26, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0110);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_rol_a_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x2a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0111);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_rol_zero_page_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x26, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0111);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_rol_a_zero_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x2a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0000;
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0001);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_rol_zero_page_zero_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x26, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0000);
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0001);
        assert_eq!(cpu.status,0b0010_0000);
    }

    // ROR
    #[test]
    fn test_ror_a() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x6a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0010;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0001);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_ror_zero_page() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x66, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0010);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0001);
        assert_eq!(cpu.status,0b0010_0000);
    }

    #[test]
    fn test_ror_a_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x6a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.run();
        assert_eq!(cpu.register_a, 0b0000_0001);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_ror_zero_page_occur_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x66, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b0000_0001);
        assert_eq!(cpu.status,0b0010_0001);
    }

    #[test]
    fn test_ror_a_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x6a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0011;
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0b1000_0001);
        assert_eq!(cpu.status,0b1010_0001);
    }

    #[test]
    fn test_ror_zero_page_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x66, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0011);
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b1000_0001);
        assert_eq!(cpu.status,0b1010_0001);
    }

    #[test]
    fn test_ror_a_zero_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x6a,0x00]);
        cpu.reset();
        cpu.register_a=0b0000_0000;
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.register_a, 0b1000_0000);
        assert_eq!(cpu.status,0b1010_0000);
    }

    #[test]
    fn test_ror_zero_page_zero_with_carry() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x66, 0b0000_0001, 0x00]);
        cpu.reset();
        cpu.mem_write(0x0001,0b0000_0000);
        cpu.status=cpu.status|0b000_0001;
        cpu.run();
        assert_eq!(cpu.mem_read(0x0001),0b1000_0000);
        assert_eq!(cpu.status,0b1010_0000);
    }
    
    // JMP
    #[test]
    fn test_jmp() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x4c, 0x30,0x40,0x00]);
        cpu.reset();
        cpu.mem_write(0x4030, 0xa9);//0xad=LDA(Absolute)
        cpu.mem_write(0x4031, 0x22);
        cpu.run();
        assert_eq!(cpu.status,0b0010_0000);
        //assert_eq!(cpu.program_counter,0x4032);//0x00があるのでややこしい
        assert_eq!(cpu.register_a,0x22);
    }

    #[test]
    fn test_jmp_indirect() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x6c, 0x30,0x40,0x00]);
        cpu.reset();
        cpu.mem_write(0x4030, 0x01);
        cpu.mem_write(0x4031, 0x02);
        cpu.mem_write(0x0201, 0xa9);
        cpu.mem_write(0x0202, 0x66);
        cpu.run();
        assert_eq!(cpu.status,0b0010_0000);
        //assert_eq!(cpu.program_counter,0x0203);
        assert_eq!(cpu.register_a,0x66);
    }

    // JSR
    #[test]
    fn test_jsr() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x20, 0x30,0x40,0x00]);
        cpu.reset();
        cpu.mem_write(0x4030, 0xa9);
        cpu.mem_write(0x4031, 0x02);
        cpu.run();
        assert_eq!(cpu.status,0b0010_0000);
        assert_eq!(cpu.register_a,0x02);

    }


    // JSR & RTS
    #[test]
    fn test_jsr_and_rts() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x20, 0x30, 0x40, 0x69,0x02,0x00]);
        cpu.reset();
        cpu.mem_write(0x4030, 0xa9);//LDA
        cpu.mem_write(0x4031, 0x77); 
        cpu.mem_write(0x4032, 0x60);//RTS
        cpu.mem_write(0x4033, 0x00); 
        cpu.run();
        assert_eq!(cpu.status,0b0010_0000);
        assert_eq!(cpu.register_a,0x79);



    }


    // PHP
    #[test]
    fn test_php() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x08,0x00]);
        cpu.reset();
        cpu.status=cpu.status|0b1100_0000;
        cpu.run();
        assert_eq!(cpu.status,0b1110_0000);
        assert_eq!(cpu.stack_pointer, 0xFc);
        assert_eq!(cpu.mem_read(0x01Fc), 0b1111_0000);
    }

    // PLP
    #[test]
    fn test_plp() {
        let mut cpu=CPU::new();
        cpu.load(vec![0x28,0x00]);
        cpu.reset();
        cpu.push(cpu.status|0b0000_0011);
        cpu.run();
        assert_eq!(cpu.stack_pointer, 0xFD);
        assert_eq!(cpu.status, 0b0010_0011);

    }

    // // PHP & PLP//BEQを実装したらやる
    // #[test]
    // fn test_plp_and_plp() {
    //     let mut cpu=CPU::new();
    //     cpu.load(vec![0x08, 0xa9, 0xF0, 0x28,0x00]);
    //     cpu.reset();
    //     cpu.status=cpu.status|0b0100_0001;
    //     cpu.run();
    //     assert_eq!(cpu.status,0b1110_0000);
    //     assert_eq!(cpu.stack_pointer, 0xFD);
    //     assert_eq!(cpu.status, 0b0010_0011);


    //     let cpu = run(vec![0x08, 0xa9, 0xF0, 0x28, 0x00], |cpu| {
    //         cpu.status = FLAG_OVERFLOW | FLAG_CARRY;
    //     });
    //     assert_eq!(cpu.register_a, 0xF0);
    //     assert_status(&cpu, FLAG_OVERFLOW | FLAG_CARRY);
    //     assert_eq!(cpu.stack_pointer, 0xFF);
    //     assert_eq!(cpu.program_counter, 0x8005);
    // }
}

