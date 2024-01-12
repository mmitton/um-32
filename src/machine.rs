use std::{
    collections::VecDeque,
    io::{Read, Write},
};

use crate::Error;

pub struct Machine {
    pc: u32,
    registers: [u32; 8],
    arrays: Vec<Option<Vec<u32>>>,
    free_arrays: Vec<(u32, Vec<u32>)>,
    input: VecDeque<char>,
    inst: [(u64, u64); 14],
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            pc: 0,
            registers: [0; 8],
            free_arrays: Vec::new(),
            arrays: vec![Some(Vec::new())],
            input: VecDeque::new(),
            inst: Default::default(),
        }
    }
}

impl Machine {
    pub fn add_input(&mut self, input: &str) {
        self.input.extend(input.chars());
    }

    pub fn extend_from(&mut self, mut r: impl Read) -> Result<(), Error> {
        let mut array = Vec::new();
        r.read_to_end(&mut array)?;
        let mut array = array
            .chunks(4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .collect();

        match self.arrays.get_mut(0) {
            Some(Some(a)) => {
                a.append(&mut array);
            }
            _ => {
                return Err(Error::InactiveArray {
                    pc: self.pc,
                    array: 0,
                })
            }
        }

        Ok(())
    }

    fn read_value(&self, array: u32, offset: u32) -> Result<u32, Error> {
        match self.arrays.get(array as usize) {
            Some(Some(a)) => match a.get(offset as usize) {
                Some(v) => Ok(*v),
                None => Err(Error::OutOfBounds {
                    pc: self.pc,
                    array,
                    offset,
                    len: a.len() as u32,
                }),
            },
            _ => Err(Error::InactiveArray { pc: self.pc, array }),
        }
    }

    fn write_value(&mut self, array: u32, offset: u32, val: u32) -> Result<(), Error> {
        match self.arrays.get_mut(array as usize) {
            Some(Some(a)) => match a.get_mut(offset as usize) {
                Some(v) => {
                    *v = val;
                    Ok(())
                }
                None => Err(Error::OutOfBounds {
                    pc: self.pc,
                    array,
                    offset,
                    len: a.len() as u32,
                }),
            },
            _ => Err(Error::InactiveArray { pc: self.pc, array }),
        }
    }

    fn read_reg(&mut self, reg: u32) -> u32 {
        unsafe { *self.registers.get_unchecked_mut(reg as usize) }
    }

    fn write_reg(&mut self, reg: u32, val: u32) {
        unsafe {
            *self.registers.get_unchecked_mut(reg as usize) = val;
        }
    }

    fn _rdtscp() -> u64 {
        unsafe {
            let mut aux = 0;
            core::arch::x86_64::__rdtscp(&mut aux)
        }
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let mut stdin = std::io::stdin().lock();
        let mut stdout = std::io::stdout().lock();
        const DEBUG: bool = false;
        const INSTRUMENT: bool = false;
        loop {
            let inst = self.read_value(0, self.pc)?;
            let op = inst >> 28;
            let start = if INSTRUMENT { Self::_rdtscp() } else { 0 };

            let (a, b, c) = if op < 13 {
                let a = (inst >> 6) & 0b111;
                let b = (inst >> 3) & 0b111;
                let c = inst & 0b111;
                (a, b, c)
            } else {
                let a = (inst >> 25) & 0b111;
                let b = inst & !(!0 << 25);
                (a, b, 0)
            };
            macro_rules! debug {
                ($($tt:tt)*) => {
                    if DEBUG {
                        write!(stdout,
                            "pc:{pc:04x}  op:{op:02}  a:{a:02x}  b:{b:02x}  c:{c:02x}  regs:{regs:02x?}  inst:{inst:032b}  ",
                            pc = self.pc,
                            regs = self.registers)?;
                        writeln!(stdout, $($tt)*)?;
                    }
                };
            }

            match op {
                0 => {
                    /*
                        #0. Conditional Move.

                        The register A receives the value in register B,
                        unless the register C contains 0.
                    */
                    debug!("IF REG[{c}], REG[{a}] = REG[{b}]");
                    let val = if self.read_reg(c) != 0 {
                        self.read_reg(b)
                    } else {
                        self.read_reg(a)
                    };
                    self.write_reg(a, val);
                    self.pc += 1;
                }

                1 => {
                    /*
                        #1. Array Index.

                        The register A receives the value stored at offset
                        in register C in the array identified by B.
                    */
                    debug!("REG[{a}] = ARRAY[REG[{b}], REG[{c}]]");
                    let b = self.read_reg(b);
                    let c = self.read_reg(c);
                    let val = self.read_value(b, c)?;
                    self.write_reg(a, val);
                    self.pc += 1;
                }

                2 => {
                    /*
                        #2. Array Amendment.

                        The array identified by A is amended at the offset
                        in register B to store the value in register C.
                    */
                    debug!("ARRAY[REG[{a}], REG[{b}]] = REG[{c}]");
                    let a = self.read_reg(a);
                    let b = self.read_reg(b);
                    let c = self.read_reg(c);
                    self.write_value(a, b, c)?;
                    self.pc += 1;
                }

                3 => {
                    /*
                        #3. Addition.

                        The register A receives the value in register B plus
                        the value in register C, modulo 2^32.
                    */
                    debug!("REG[{a}] = REG[{b}] + REG[{c}]");
                    let val = self.read_reg(b).wrapping_add(self.read_reg(c));
                    self.write_reg(a, val);
                    self.pc += 1;
                }

                4 => {
                    /*
                        #4. Multiplication.

                        The register A receives the value in register B times
                        the value in register C, modulo 2^32.
                    */
                    debug!("REG[{a}] = REG[{b}] * REG[{c}]");
                    let val = self.read_reg(b).wrapping_mul(self.read_reg(c));
                    self.write_reg(a, val);
                    self.pc += 1;
                }

                5 => {
                    /*
                        #5. Division.

                        The register A receives the value in register B
                        divided by the value in register C, if any, where
                        each quantity is treated as an unsigned 32 bit number.
                    */
                    debug!("REG[{a}] = REG[{b}] / REG[{c}]");
                    let divisor = self.read_reg(c);
                    if divisor == 0 {
                        return Err(Error::DivisionByZero { pc: self.pc });
                    }
                    let val = self.read_reg(b) / divisor;
                    self.write_reg(a, val);
                    self.pc += 1;
                }
                6 => {
                    /*
                        #6. Not-And.

                        Each bit in the register A receives the 1 bit if
                        either register B or register C has a 0 bit in that
                        position.  Otherwise the bit in register A receives
                        the 0 bit.
                    */
                    debug!("REG[{a}] = !(REG[{b}] & REG[{c}])");
                    let val = !(self.read_reg(b) & self.read_reg(c));
                    self.write_reg(a, val);
                    self.pc += 1;
                }

                7 => {
                    /*
                        #7. Halt.

                        The universal machine stops computation.
                    */
                    debug!("HALT");
                    break;
                }

                8 => {
                    /*
                        #8. Allocation.

                        A new array is created with a capacity of platters
                        commensurate to the value in the register C. This
                        new array is initialized entirely with platters
                        holding the value 0. A bit pattern not consisting of
                        exclusively the 0 bit, and that identifies no other
                        active allocated array, is placed in the B register.
                    */
                    debug!("REG[{b}] = allocate REG[{c}] words");
                    let cap = self.read_reg(c) as usize;
                    let array = if let Some((idx, mut mem)) = self.free_arrays.pop() {
                        mem.resize(cap, 0);
                        mem.fill(0);
                        self.arrays[idx as usize] = Some(mem);
                        idx
                    } else {
                        self.arrays.push(Some(vec![0; cap]));
                        self.arrays.len() as u32 - 1
                    };
                    self.write_reg(b, array);
                    self.pc += 1;
                }

                9 => {
                    /*
                        #9. Abandonment.

                        The array identified by the register C is abandoned.
                        Future allocations may then reuse that identifier.
                    */
                    debug!("deallocate REGS[{c}]");
                    let array = self.read_reg(c);
                    let mem = match self.arrays.get_mut(array as usize) {
                        Some(x @ Some(_)) => x.take().unwrap(),
                        _ => return Err(Error::InactiveArray { pc: self.pc, array }),
                    };
                    self.free_arrays.push((array, mem));
                    self.pc += 1;
                }

                10 => {
                    /*
                        #10. Output.

                        The value in the register C is displayed on the console
                        immediately. Only values between and including 0 and 255
                        are allowed.
                    */
                    debug!("Output REGS[{c}]");
                    let ch = self.read_reg(c);
                    if ch > 255 {
                        return Err(Error::InvalidChar { pc: self.pc, ch });
                    }
                    stdout.write_all(&[ch as u8])?;
                    stdout.flush()?;

                    self.pc += 1;
                }

                11 => {
                    /*
                        #11. Input.

                        The universal machine waits for input on the console.
                        When input arrives, the register C is loaded with the
                        input, which must be between and including 0 and 255.
                        If the end of input has been signaled, then the
                        register C is endowed with a uniform value pattern
                        where every place is pregnant with the 1 bit.
                    */
                    debug!("REGS[{c}] = input");
                    let ch = if let Some(ch) = self.input.pop_front() {
                        ch
                    } else {
                        let mut buf = [0];
                        stdin.read_exact(&mut buf)?;
                        buf[0] as char
                    };
                    stdout.write_all(&[ch as u8])?;
                    stdout.flush()?;
                    self.write_reg(c, ch as u32);
                    self.pc += 1;
                }

                12 => {
                    /*
                        #12. Load Program.

                        The array identified by the B register is duplicated
                        and the duplicate shall replace the '0' array,
                        regardless of size. The execution finger is placed
                        to indicate the platter of this array that is
                        described by the offset given in C, where the value
                        0 denotes the first platter, 1 the second, et
                        cetera.

                        The '0' array shall be the most sublime choice for
                        loading, and shall be handled with the utmost
                        velocity.
                    */
                    debug!("program load: duplicate memory in REG[{b}] into code space, and set instruction pointer to REG[{c}]");
                    let array = self.read_reg(b);
                    if array == 0 && self.read_reg(c) == self.pc {
                        return Err(Error::InfiniteLoop { pc: self.pc });
                    }
                    if array != 0 {
                        match self.arrays.get(array as usize) {
                            Some(Some(a)) => {
                                let a: Vec<u32> = a.clone();
                                self.arrays[0] = Some(a);
                            }
                            _ => return Err(Error::InactiveArray { pc: self.pc, array }),
                        }
                    }
                    self.pc = self.read_reg(c);
                }

                13 => {
                    /*
                        #13. Orthography.

                        The value indicated is loaded into the register A
                        forthwith.
                    */
                    debug!("REG[{a}] = {b}");
                    self.write_reg(a, b);
                    self.pc += 1;
                }

                _ => return Err(Error::InvalidOp { pc: self.pc, op }),
            }

            if INSTRUMENT {
                let end = Self::_rdtscp();
                unsafe {
                    let inst = self.inst.get_unchecked_mut(op as usize);
                    inst.0 += end - start;
                    inst.1 += 1;
                }
            }
        }

        if INSTRUMENT {
            for (i, (time, cnt)) in self.inst.iter().enumerate() {
                let avg = *time as f64 / *cnt as f64;
                writeln!(
                    stdout,
                    "INST {i:02}:  Total cycles: {time:15}  Cnt: {cnt:10}  Avg Cycles: {avg:2.2}"
                )?;
            }
        }

        Ok(())
    }
}
