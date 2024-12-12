use common::{CpuStats, CpuTrait, ExecutionError, RunMode};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::FromPrimitive;

const CYCLES_WRITE_MEMORY: usize = 1;
const CYCLES_ACCESS_L1: usize = 3;
const CYCLES_ACCESS_L2: usize = 11;
const CYCLES_ACCESS_L3: usize = 50;
const CYCLES_ACCESS_MEMORY: usize = 125;

#[derive(Debug, PartialEq, PartialOrd, Copy, Clone, Hash, Eq, Ord, FromPrimitive, ToPrimitive)]
#[repr(u32)]
pub enum Bytecode {
    Nop = 0x00,
    LoadValue = 0x01,
    LoadMemory = 0x02,
    Store = 0x03,
    PushValue = 0x04,
    PushReg = 0x05,
    Pop = 0x06,
    Add = 0x10,
    Call = 0xF0,
    Ret = 0xF1,
    RetReg = 0xF2,
    Jmp = 0xF3,
    Inspect = 0xFE,
    Halt = 0xFFFFFFFF,
}

#[derive(Debug)]
pub struct NativeCpu {
    memory: Vec<u32>,
    registers: Vec<u32>,
    instruction_pointer: u32,
    stack_pointer: u32, // Stack pointer
    verbose: bool,
    cycles: usize, // Performance counter for cycles
    l1_start: u32,
    l1_size: u32,
    l2_start: u32,
    l2_size: u32,
    l3_start: u32,
    l3_size: u32,
    access_count_l1: usize,
    access_count_l2: usize,
    access_count_l3: usize,
    access_count_memory: usize,
}

impl CpuTrait for NativeCpu {
    type Size = u32;

    fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }

    fn load_memory(&mut self, address: Self::Size, memory: &[Self::Size]) {
        let len = self.memory.len().min(memory.len());
        self.memory[address as usize..address as usize + len].copy_from_slice(&memory[..len]);
    }

    fn execute(&mut self, run_mode: RunMode) -> Result<CpuStats, ExecutionError> {
        match run_mode {
            RunMode::Run => self.run(-1),
            RunMode::RunFor(cycles) => self.run(cycles),
            _ => Err(ExecutionError::NotImplemented),
        }
    }

    fn get_registers(&self) -> &[Self::Size] {
        &self.registers
    }

    fn get_memory(&self) -> &[Self::Size] {
        &self.memory
    }
}

impl NativeCpu {
    pub fn new(memory_size: u32, registers: u8) -> Self {
        Self {
            memory: vec![0; memory_size as usize],
            registers: vec![0; registers as usize],
            instruction_pointer: 0,
            stack_pointer: memory_size - 1, // Stack starts at the top of memory
            verbose: false,
            cycles: 0,
            l1_start: 0,
            l1_size: 64 * 1024, // 64 KB L1 cache
            l2_start: 64 * 1024,
            l2_size: 256 * 1024, // 256 KB L2 cache
            l3_start: 320 * 1024,
            l3_size: 1024 * 1024, // 1 MB L3 cache
            access_count_l1: 0,
            access_count_l2: 0,
            access_count_l3: 0,
            access_count_memory: 0,
        }
    }

    fn run(&mut self, cycles: isize) -> Result<CpuStats, ExecutionError> {
        let mut executed = 0;

        while (cycles < 0 || executed < cycles)
            && (self.instruction_pointer as usize) < self.memory.len()
        {
            let opcode = self.read_memory(self.instruction_pointer)?;
            self.instruction_pointer += 1;

            match Bytecode::from_u32(opcode) {
                Some(Bytecode::LoadValue) => {
                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    let imm = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("SET R{}, {}", reg, imm);
                    }

                    self.registers[reg as usize] = imm;
                    self.cycles += 2;
                }
                Some(Bytecode::LoadMemory) => {
                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    let addr = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("LOAD R{}, @{:#02x}", reg, addr);
                    }

                    self.registers[reg as usize] = self.read_memory(addr)?;
                    self.cycles += 2;
                }
                Some(Bytecode::Store) => {
                    let addr = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("STORE @{:#02x}, R{}", addr, reg);
                    }

                    self.write_memory(addr, self.registers[reg as usize])?;
                    self.cycles += 2;
                }
                Some(Bytecode::Inspect) => {
                    let addr = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    println!("INSPECT @{:#02x} = {}", addr, self.read_memory(addr)?);
                }
                Some(Bytecode::Add) => {
                    let reg1 = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    let reg2 = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!(
                            "ADD R{}({}), R{}({})",
                            reg1,
                            self.registers[reg1 as usize],
                            reg2,
                            self.registers[reg2 as usize]
                        );
                    }

                    self.registers[reg1 as usize] =
                        self.registers[reg1 as usize].wrapping_add(self.registers[reg2 as usize]);
                    self.cycles += 1;
                }
                Some(Bytecode::Jmp) => {
                    let imm = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer = imm;

                    if self.verbose {
                        println!("JMP {}", imm);
                    }

                    self.cycles += 2;
                }
                Some(Bytecode::PushValue) => {
                    let imm = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("PUSH {}", imm);
                    }

                    self.push_stack(imm)?;

                    self.cycles += 2;
                }
                Some(Bytecode::PushReg) => {
                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("PUSH R{}", reg);
                    }

                    let val = self.registers[reg as usize];

                    self.push_stack(val)?;

                    self.cycles += 2;
                }
                Some(Bytecode::Pop) => {
                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("POP R{}", reg);
                    }

                    self.registers[reg as usize] = self.pop_stack()?;

                    self.cycles += 2;
                }
                Some(Bytecode::Call) => {
                    let target = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    if self.verbose {
                        println!("CALL @{:#02x}", target);
                    }

                    // Push the current PC onto the stack as the return address
                    self.push_stack(self.instruction_pointer)?;

                    // Jump to the target address
                    self.instruction_pointer = target;

                    self.cycles += 25;
                }
                Some(Bytecode::RetReg) => {
                    let reg = self.read_memory(self.instruction_pointer)?;
                    self.instruction_pointer += 1;

                    // Pop the return address from the stack
                    let return_address = self.pop_stack()?;

                    if self.verbose {
                        println!("RET R{} to @{:#02x}", reg, return_address);
                    }

                    self.push_stack(self.registers[reg as usize])?;

                    self.instruction_pointer = return_address;
                    self.cycles += 2;
                }
                Some(Bytecode::Ret) => {
                    // Pop the return address from the stack
                    let return_address = self.pop_stack()?;

                    if self.verbose {
                        println!("RET to @{:#02x}", return_address);
                    }

                    self.instruction_pointer = return_address;
                    self.cycles += 23;
                }
                Some(Bytecode::Halt) => {
                    if self.verbose {
                        println!("HALT");
                    }
                    break;
                }
                Some(Bytecode::Nop) => {
                    if self.verbose {
                        println!("NOP");
                    }
                }
                None => {
                    println!("Unknown opcode: {:X}", opcode);
                    break;
                }
            }
            executed += 1;
        }
        Ok(CpuStats {
            cycles: self.cycles,
        })
    }

    fn update_memory_cycles(&mut self, address: u32) {
        if address >= self.l1_start && address < self.l1_start + self.l1_size {
            self.access_count_l1 += 1;
            self.cycles += CYCLES_ACCESS_L1;
        } else if address >= self.l2_start && address < self.l2_start + self.l2_size {
            self.access_count_l2 += 1;
            self.cycles += CYCLES_ACCESS_L2;
        } else if address >= self.l3_start && address < self.l3_start + self.l3_size {
            self.access_count_l3 += 1;
            self.cycles += CYCLES_ACCESS_L3;
        } else {
            self.access_count_memory += 1;
            self.cycles += CYCLES_ACCESS_MEMORY;
        }
    }

    fn valid_address(&self, address: u32) -> Result<(), ExecutionError> {
        if (address as usize) < self.memory.len() {
            Ok(())
        } else {
            Err(ExecutionError::InvalidMemoryLocation)
        }
    }

    fn read_memory(&mut self, address: u32) -> Result<u32, ExecutionError> {
        self.valid_address(address)?;
        self.update_memory_cycles(address);
        Ok(self.memory[address as usize])
    }

    fn write_memory(&mut self, address: u32, value: u32) -> Result<(), ExecutionError> {
        self.valid_address(address)?;
        self.cycles += CYCLES_WRITE_MEMORY;
        self.memory[address as usize] = value;
        Ok(())
    }

    fn push_stack(&mut self, value: u32) -> Result<(), ExecutionError> {
        if self.stack_pointer == 0 {
            return Err(ExecutionError::StackOverflow);
        }

        self.write_memory(self.stack_pointer, value)?;
        self.stack_pointer -= 1;
        Ok(())
    }

    fn pop_stack(&mut self) -> Result<u32, ExecutionError> {
        if self.stack_pointer as usize >= self.memory.len() - 1 {
            return Err(ExecutionError::StackUnderflow);
        }

        self.stack_pointer += 1;
        self.read_memory(self.stack_pointer)
    }
}
