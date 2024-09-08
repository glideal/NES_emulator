pub struct CPU{
    pub register_a:u8,//Acumulator
    pub status: u8,/*flag
    |Negative|oVerflow| |Break command|
    Decimal mode flag|Interpret disable|Zero flag|Carry flag
    */
    pub program_counter:u16,

    pub register_x:u8,
}

impl CPU{
    pub fn new()->Self{
        CPU { 
            register_a: 0, 
            status: 0, //0b0000_0000
            program_counter: 0, 
            register_x:0,
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


    pub fn interpret(&mut self, program:Vec<u8>){
        self.program_counter=0;
        loop{
            let opscode =program[self.program_counter as usize];
            self.program_counter+=1;

            match opscode{
                0xA9=>{//LDA
                    let param=program[self.program_counter as usize];
                    self.program_counter+=1;

                    self.lda(param);
                }
                
                0xAA=>{
                    self.tax()
                }

                0x00=>{
                    return;
                }
                _=>todo!()
            }
        }
    }
}


fn main(){
    println!("hello world");
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
    fn test_0xaa_tax(){
        let mut cpu=CPU::new();
        cpu.register_a=10;
        cpu.interpret(vec![0xaa,0x00]);
        assert_eq!(cpu.register_x,10);
    }
}

