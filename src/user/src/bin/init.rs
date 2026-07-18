#![no_std]
#![no_main]

use user::{entry, println, print};
use user::stdlib::syscalls::{sys_readline, fork, exec, wait};

// \REVIEW: There are several ways to proceed with it.  One way would be to
// simply give out more stack pages to programs.  Another way can have us
// implement C-style strings in Rust so an array of u8 with a start pointer and
// an offset basically.  Any path that you take, just know that these values are
// yours to tweak to perfection:
const MAX_ARGS: usize = 16;
const MAX_COMMANDS: usize = 8;

const INPUT_SIZE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Token<'a> {
    Word(&'a str),
    Pipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexError {
    UnterminatedString,
}

pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0
        }
    }

    pub fn next_token(&mut self) -> Result<Option<Token<'a>>, LexError> {
        let bytes = self.input.as_bytes();

        // trim whitespaces
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }

        if self.pos >= bytes.len() {
            return Ok(None);
        }

        match bytes[self.pos] {
            b'|' => {
                self.pos += 1;
                Ok(Some(Token::Pipe))
            }

            b'"' => {
                self.pos += 1;
                let start = self.pos;
                while self.pos < bytes.len() && bytes[self.pos] != b'"' {
                    self.pos += 1;
                }

                if self.pos == bytes.len() {
                    return Err(LexError::UnterminatedString);
                }

                let word = &self.input[start..self.pos];
                self.pos += 1;

                Ok(Some(Token::Word(word)))
            }

            _ => {
                let start = self.pos;
                while self.pos < bytes.len() {
                    match bytes[self.pos] {
                        b'|' | b'"' => break,
                        c if c.is_ascii_whitespace() => break,
                        _ => self.pos += 1,
                    }
                }

                Ok(Some(Token::Word(&self.input[start..self.pos])))
            }
        }
    }
}

struct Command<'a> {
    argc: usize,
    argv: [&'a str; MAX_ARGS],
}

pub struct ExecutionPipeline<'a> {
    count: usize,
    commands: [Command<'a>; MAX_COMMANDS],
}

impl<'a> Command<'a> {
    pub const fn new() -> Self {
        Self {
            argc: 0,
            argv: [""; MAX_ARGS],
        }
    }
}

impl<'a> ExecutionPipeline<'a> {
    pub const fn new() -> Self {
        Self {
            count: 0,
            commands: [const { Command::new() }; MAX_COMMANDS],
        }
    }
}

#[derive(Debug)]
pub enum ParserError {
    Lex(LexError),
    EmptyCommand,
    TooManyArguments,
    TooManyCommands,
}

pub struct Parser<'a> {
    lexer: Lexer<'a>,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            lexer: Lexer::new(input),
        }
    }

    pub fn parse(&mut self) -> Result<ExecutionPipeline<'a>, ParserError> {
        let mut pipeline = ExecutionPipeline::new();
        let mut current = Command::new();

        loop {
            let token = match self.lexer.next_token() {
                Ok(token) => token,
                Err(e) => return Err(ParserError::Lex(e)),
            };

            match token {
                Some(Token::Word(word)) => {
                    if current.argc >= MAX_ARGS {
                        return Err(ParserError::TooManyArguments);
                    }

                    current.argv[current.argc] = word;
                    current.argc += 1;
                }

                Some(Token::Pipe) => {
                    if current.argc == 0 {
                        return Err(ParserError::EmptyCommand);
                    }

                    if pipeline.count >= MAX_COMMANDS {
                        return Err(ParserError::TooManyCommands)
                    }

                    pipeline.commands[pipeline.count] = current;
                    pipeline.count += 1;

                    current = Command::new();
                }

                None => {
                    break;
                }
            }
        }

        if current.argc == 0 {
            return Err(ParserError::EmptyCommand);
        }

        if pipeline.count >= MAX_COMMANDS {
            return Err(ParserError::TooManyCommands);
        }

        pipeline.commands[pipeline.count] = current;
        pipeline.count += 1;

        Ok(pipeline)
    }
}

// @Todo This needs a lot of work after we make the syscall api better.
// That is why execs and forks are not utilised at the moment
fn execute_command(command: &Command) {
    if command.argc == 0 {
        return;
    }

    /*
    let _ = println!("argc={}", command.argc);
    for i in 0..command.argc {
        let _ = println!("argv[{}]='{}'", i, command.argv[i]);
}
    */
    
    let program = command.argv[0];
    let args = &command.argv[1..command.argc];

    match fork() {
        Ok(0) => {
            // child
            if exec(program, args).is_err() {
                let _ = println!("{}: command not found", program);
            }

            user::stdlib::syscalls::exit(1);
        }

        Ok(pid) => {
            // parent
            let _ = wait(Some(pid));
        }

        Err(_) => {
            let _ = println!("fork failed!");
        }
    }
}

fn execute_pipeline(pipeline: &ExecutionPipeline) {
    for i in 0..pipeline.count {
        execute_command(&pipeline.commands[i]);
    }
}

fn main() {
    let mut buf = [0u8; INPUT_SIZE];
    
    loop {
        let _ = print!("$ ");
        let n = sys_readline(&mut buf);

        let input = match core::str::from_utf8(&buf[..n]) {
            Ok(s) => s,
            Err(_) => {
                let _ = println!("invalid utf-8!");
                continue;
            }
        };
        
        let mut parser = Parser::new(input);
        match parser.parse() {
            Ok(pipeline) => {
                execute_pipeline(&pipeline);
            }
            
            Err(e) => {
                let _ = println!("parser error: {:?}", e);
            }
        }
    }
}

entry!(main);

