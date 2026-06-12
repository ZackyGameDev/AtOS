use crate::kernel::peripherals::Uart;

pub fn getch() -> u8 {
    let uart = Uart;

    loop {
	let c = uart.read_byte();

	if c == b'\r' {
	    uart.write_byte(b'\n');
	    return b'\n';
	}

	if c == b'\n' {
	    continue;
	}

	uart.write_byte(c);
	return c;
    }
}

pub fn getline(buf: &mut [u8]) -> usize {
    let mut r = 0;

    while r < buf.len() {
	let c = getch();

	buf[r] = c;
	r += 1;

	if c == b'\n' {
	    break;
	}
    }

    r
}

