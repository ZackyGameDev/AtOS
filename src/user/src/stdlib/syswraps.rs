use crate::stdlib::syscalls::{fork, exec, wait};

pub fn spawn(path: &str, args: &[&str]) -> Result<(), &'static str> {
    match fork() {
        Ok(fc) => {
            if fc == 0 {
                exec(path, args)?;
            } else {
                wait(Some(fc))?;
            }
        }
        Err(_) => {
            return Err("fork failed");
        }
    }

    Ok(())
}