use std::convert::TryInto;

#[derive(Debug, PartialEq)]
enum SectionType
{
    Data,
    Code
}

#[derive(Default, Debug)]
struct Memory
{
    memory: Vec<u8>,
    // TODO: add a param for permissions too
    // SectionType, Start, End
    regions: Vec<(SectionType, usize, usize)>
}

impl Memory
{
    fn new(size: Option<usize>, regions: Option<Vec<(SectionType, usize, usize)>>) -> Self
    {
        // TODO: check to see if the ranges are valid, and the sections types are unique
        Self {
            memory: vec![0; clamp(size.unwrap_or(0), std::u32::MAX as usize)],
            // TODO: check to see if two different SectionTypes are overlapping
            regions: regions.unwrap_or(vec![(SectionType::Data, 0, size.unwrap_or(0))])
        }
    }

    /// gets section ranges filtered by type
    fn get_range(&self, stype: SectionType) -> Vec<(usize, usize)>
    {
        self.regions
            .iter()
            .filter(|f| f.0 == stype)
            .map(|m| (m.1, m.2))
            .collect()
    }
}

#[derive(Default, Debug)]
struct Registers
{
    // general purpose registers
    r1: i32, r2: i32, r3: i32, r4: i32,

    // instruction pointer
    rip: i32
}

#[derive(Default, Debug)]
struct Flags
{
    // zero flag
    zf: bool, 
    // overflow flag
    of: bool, 
    // trap flag (DEBUG)
    tf: bool, 
}

#[derive(Default)]
struct CPU
{
    ram: Memory,
    registers: Registers,
    flags: Flags,
}

impl CPU
{
    fn new(size: Option<usize>, regions: Option<Vec<(SectionType, usize, usize)>>) -> Self 
    { 
        Self { 
            ram: Memory::new(size, regions),
            ..Default::default() 
        } 
    }

    /// prints a hexdump to screen (bytes per line determined by @lim param).
    #[allow(dead_code)]
    fn dump(&self, lim: Option<usize>) 
    {
        for i in (0..self.ram.memory.len()).step_by(lim.unwrap_or(8)) {
            print!("0x{:X}:\t", i);
            for x in 0..lim.unwrap_or(8) { 
                if let Some(e) = self.ram.memory.get(i + x) {
                    print!("{:02X} ", e);
                }
            }
            println!();
        }
    }

    /// appends an instruction onto the code region in memory
    fn append(&mut self, instruction: Instruction) 
    { 
        let range = self.ram.get_range(SectionType::Code);

        // iterates over the range of the first element that contained the code section
        for byte in (range[0].0..range[0].1).step_by(8)
        {
            if self.ram.memory[byte] == 0 {
                // there could possibly be a nicer way to memcpy here
                for i in 0..8 { self.ram.memory[byte + i] = instruction.as_bytes()[i]; }
                return;
            }
        }
        // log: there was no free space to put instruction
    } 

    /// gets 8 bytes from memory
    fn fetch(&self, start: i32) -> Vec<u8>
    {
       self.ram.memory[(start as usize)..start as usize + 8].to_vec()
    }

    /// turns the 8 bytes from fetch into an instruction
    fn decode(bytes: Vec<u8>) -> Option<Instruction>
    { 
        Some(Instruction::parse(bytes)?) 
    }
    
    /// executes the instruction from decode
    fn execute(&mut self, instruction: Instruction) -> Result<(), String> 
    {
        match instruction.mnemonic {
            0x1 => { instruction.mov(self)? },
            _ => { return Err(format!("error: invalid mnemonic {:02X} address 0x{:X}", instruction.mnemonic, self.registers.rip)) }
        }
        Ok(())
    }

    /// combines fetch, decode and execute on a loop for each instruction
    fn run(&mut self) -> Result<(), String>
    {
        let range = self.ram.get_range(SectionType::Code);
        
        // iterates over the range of the first element that contained the code section
        for _ in (range[0].0..range[0].1).step_by(8) {
            // if the decode function parsed a valid instruction
            if let Some(i) = CPU::decode(self.fetch(self.registers.rip)) {
                self.execute(i)?;
                self.registers.rip += 8;
            } else { return Err("error: instruction couldn't be decoded".to_string()) }
        }
        Ok(())
    }
}

/// creates 8 byte instructions
#[derive(Debug)]
struct Instruction
{
    // [00] 00 00 00 | 00 00 00 00
    mnemonic: u8,
    // 00 [00] 00 00 | 00 00 00 00
    modifier: u8,
    // 00 00 [00] 00 | 00 00 00 00
    register_from: u8,
    // 00 00 00 [00] | 00 00 00 00
    register_to: u8,
    // 00 00 00 00 | [00 00 00 00]
    data: u32,
}

impl Instruction
{
    fn new(mnemonic: u8, modifier: u8, register_from: u8, register_to: u8, data: u32) -> Self { Self { mnemonic, modifier, register_from, register_to, data } }

    /// returns a vector of bytes from an Instruction
    fn as_bytes(&self) -> Vec<u8> 
    { 
        let mut prelim = vec![self.mnemonic, self.modifier, self.register_from, self.register_to];
        for e in self.data.to_be_bytes().to_vec() { prelim.push(e); }
        prelim
    }

    /// turns a vector of 8 bytes into an Instruction
    fn parse(mut bytes: Vec<u8>) -> Option<Instruction>
    {
        if bytes.len() != 8 { return None; }

        let mut xinstruction = Instruction::new(0, 0, 0, 0, 0);

        for i in 0..bytes.len() {
            match i {
                0 => xinstruction.mnemonic = bytes[0], 
                1 => xinstruction.modifier = bytes[0], 
                2 => xinstruction.register_from = bytes[0],
                3 => xinstruction.register_to = bytes[0],
                _ => ()
            }

            // if this were in the match expression it would generate a warning
            if i == 4 {
                if let Ok(data) = bytes.as_slice().try_into()
                {
                    xinstruction.data = u32::from_be_bytes(data);
                    break;
                } else { return None; }
            }
            
            // shifts vector by 1, there might be a better way
            bytes = bytes[1..].to_vec();
        }

        Some(xinstruction)
    }

    fn mov(&self, ctx: &mut CPU) -> Result<(), String>
    {
        match self.modifier {
            0x0 => {
                // TODO: make registers indexable to remove redundancy
                match self.register_to {
                    0 => { ctx.registers.r1 = self.data as i32;  },
                    1 => { ctx.registers.r2 = self.data as i32;  },
                    2 => { ctx.registers.r3 = self.data as i32;  },
                    3 => { ctx.registers.r4 = self.data as i32;  },
                    _ => { return Err(format!("error: invalid register {:02X} at 0x{:X}", self.register_to, ctx.registers.rip + 3)) } 
                }

                if self.register_from != 0 { 
                    return Err(format!("error: non-zero value for unusable byte 0x{:X}", ctx.registers.rip + 2));
                }
            },
            _ => { return Err(format!("error: invalid modifier {:02X} at 0x{:X}", self.modifier, ctx.registers.rip + 1)) }, 
        };
        Ok(())
    }
}

// TODO: Add support for stack operations

fn main() 
{
    // TODO: put this in the CPU::new() method 
    let size = 1504;
    let regions = vec![(SectionType::Code, 0, size / 3), (SectionType::Data, size / 3, size)];
    let mut example = CPU::new(Some(size), Some(regions));

    example.append(Instruction::new(1, 0, 0, 1, 1337));
    
    if let Err(e) = example.run() {
        println!("\n{}\n", e);
        example.dump(None);
    }
}

fn clamp(n: usize, max: usize) -> usize
{
    if n > max { max } else { n } 
}
