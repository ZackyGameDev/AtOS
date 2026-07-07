use crate::stdlib::syscalls::{fork, exec, wait};

pub fn spawn(path: &str) -> Result<(), &'static str> {
    match fork() {
        Ok(fc) => {
            if fc == 0 {
                exec(path)?;
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