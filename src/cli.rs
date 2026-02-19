// CLI 命令定义

#[derive(Debug, PartialEq, Clone)]
pub enum Command {
    Scan,
    Read,
    Write,
    Dump,
    Status,
    Help,
    Unknown,
}

pub fn parse(input: &str) -> Command {
    let input = input.trim();
    match input {
        "SCAN" | "scan" => Command::Scan,
        "READ" | "read" => Command::Read,
        "WRITE" | "write" => Command::Write,
        "DUMP" | "dump" => Command::Dump,
        "STATUS" | "status" => Command::Status,
        "HELP" | "help" => Command::Help,
        _ => Command::Unknown,
    }
}
