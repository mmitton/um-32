use machine::Machine;

mod machine;

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    DivisionByZero {
        pc: u32,
    },
    IO(std::io::Error),
    InfiniteLoop {
        pc: u32,
    },
    InactiveArray {
        pc: u32,
        array: u32,
    },
    InvalidChar {
        pc: u32,
        ch: u32,
    },
    InvalidOp {
        pc: u32,
        op: u32,
    },
    MissingFile,
    OutOfBounds {
        pc: u32,
        array: u32,
        offset: u32,
        len: u32,
    },
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::IO(e)
    }
}

fn main() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(Error::MissingFile);
    }

    let mut machine = Machine::default();
    for file in args.iter().skip(1) {
        machine.extend_from(std::fs::File::open(file)?)?;
    }

    if args[1].ends_with("codex.umz") {
        machine.add_input("(\\b.bb)(\\v.vv)06FHPVboundvarHRAkp");
    }
    machine.run()?;

    Ok(())
}
