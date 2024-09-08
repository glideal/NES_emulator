pub struct CPU{
    pub register_a:u8,//Acumulator
    pub register_x:u8,
    pub status: u8,/*flag
    |Negative|oVerflow| |Break command|
    Decimal mode flag|Interpret disable|Zero flag|Carry flag
    */
    pub program_counter:u16,
    memory:[u8;0xFFFF],

}

impl CPU{
    pub fn new()->Self{
        CPU { 
            register_a: 0, 
            register_x:0,
            status: 0, //0b0000_0000
            program_counter: 0, 
            memory:[0;0xFFFF],
        }
    } 

    fn lda(&mut self, value:u8){
        self.register_a=value;
        self.update_zero_and_negative_flags(self.register_a);
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

    /*オペランドなどが8バイトなのに対して、アドレスは16バイト */
    fn mem_read(&self,addr:u16)->u8{
        self.memory[addr as usize]
    }

    fn mem_read_u16(&mut self, pos:u16)->u16{
        let lo=self.mem_read(pos) as u16;
        let hi =self.mem_read(pos+1) as u16;
        (hi<<8)|(lo as u16)//<<は左シフト演算子
    }

    fn mem_write(&mut self, addr:u16,data:u8){
        self.memory[addr as usize]=data;
    }

    fn mem_wtite_u16(&mut self, pos:u16,data:u16){
        let hi=(data>>8) as u8;
        let lo=(data & 0b00000000_11111111) as u8;
        self.mem_write(pos,lo);
        self.mem_write(pos+1,hi);
    }
    
    pub fn load_and_run(&mut self,program:Vec<u8>){
        self.load(program);
        self.run();
    }

    pub fn load(&mut self,program:Vec<u8>){
        self.memory[0x8000..(0x8000+program.len())].copy_from_slice(&program[..]);
        //memory[0c8000..(ox8000+program.len())]にprogram[..]の内容を格納する。
        self.program_counter=0x8000;
    }

    pub fn run(&mut self){
        loop{
            let opscode=self.mem_read(self.program_counter);
            self.program_counter+=1;
            match opscode{
                0xA9=>{//LDA
                    let param=self.mem_read(self.program_counter);
                    self.program_counter+=1;

                    self.lda(param);
                }

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
        cpu.register_a=10;
        cpu.interpret(vec![0xaa,0x00]);
        assert_eq!(cpu.register_x,10);
    }

    #[test]
    fn test_5_ops_working_together(){
        let mut cpu =CPU::new();
        cpu.interpret(vec![0xa9,0xc0,0xaa,0xe8,0x00]);

        assert_eq!(cpu.register_x,0xc1);
    }
    #[test]
    fn test_inx_ooverflow(){
        let mut cpu=CPU::new();
        cpu.register_x=0xff;
        cpu.interpret(vec![0xe8,0xe8,0x00]);

        assert_eq!(cpu.register_x,1);
    }
}

