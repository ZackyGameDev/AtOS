/* tetris.rs -- a tetris implementation and the first built-in game for AtOS */
#![no_std]
#![no_main]

use user::{entry, print, println};
use user::stdlib::syscalls::{poll_char, sleep, exit};

const BOARD_WIDTH: usize = 10;
const BOARD_HEIGHT: usize = 20;

const SHAPES: [[[u8; 4]; 4]; 7] = [
    // I
    [[0,0,0,0],
     [1,1,1,1],
     [0,0,0,0],
     [0,0,0,0]],

    // J
    [[1,0,0,0],
     [1,1,1,0],
     [0,0,0,0],
     [0,0,0,0]],

    // L
    [[0,0,1,0],
     [1,1,1,0],
     [0,0,0,0],
     [0,0,0,0]],

    // O
    [[0,1,1,0],
     [0,1,1,0],
     [0,0,0,0],
     [0,0,0,0]],

    // S
    [[0,1,1,0],
     [1,1,0,0],
     [0,0,0,0],
     [0,0,0,0]],

    // T
    [[0,1,0,0],
     [1,1,1,0],
     [0,0,0,0],
     [0,0,0,0]],
    
    // Z
    [[1,1,0,0],
     [0,1,1,0],
     [0,0,0,0],
     [0,0,0,0]],
];

struct GameState {
    board: [[u8; BOARD_WIDTH]; BOARD_HEIGHT],
    current_piece: [[u8; 4]; 4],
    piece_x: isize,
    piece_y: isize,
    score: usize,
    game_over: bool,
    rng_seed: u32, /* idk how rng works i just nicked it off from an llm */
}

impl GameState {
    fn new() -> Self {
        let mut state = Self {
            board: [[0; BOARD_WIDTH]; BOARD_HEIGHT],
            current_piece: [[0; 4]; 4],
            piece_x: 0,
            piece_y: 0,
            score: 0,
            game_over: false,
            rng_seed: 0x5EED, // Simple baseline LCG seed
        };
        state.spawn_piece();
        state
    }

    // Stack-only LCG random generator (given by an llm might need checking)
    fn next_piece_index(&mut self) -> usize {
        self.rng_seed = self.rng_seed.wrapping_mul(1103515245).wrapping_add(12345);
        ((self.rng_seed / 65536) % 7) as usize
    }

    fn spawn_piece(&mut self) {
        let shape_idx = self.next_piece_index();
        self.current_piece = SHAPES[shape_idx];
        self.piece_x = (BOARD_WIDTH as isize / 2) - 2;
        self.piece_y = 0;

        if self.check_collision(self.piece_x, self.piece_y, &self.current_piece) {
            self.game_over = true;
        }
    }

    fn check_collision(&self, next_x: isize, next_y: isize, piece: &[[u8; 4]; 4]) -> bool {
        for r in 0..4 {
            for c in 0..4 {
                if piece[r][c] != 0 {
                    let board_x = next_x + c as isize;
                    let board_y = next_y + r as isize;

                    if board_x < 0 || board_x >= BOARD_WIDTH as isize || board_y >= BOARD_HEIGHT as isize {
                        return true;
                    }
                    if board_y >= 0 && self.board[board_y as usize][board_x as usize] != 0 {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn rotate_piece(&mut self) {
        let mut rotated = [[0u8; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                rotated[c][3 - r] = self.current_piece[r][c];
            }
        }
        if !self.check_collision(self.piece_x, self.piece_y, &rotated) {
            self.current_piece = rotated;
        }
    }

    fn move_horizontal(&mut self, dir: isize) {
        if !self.check_collision(self.piece_x + dir, self.piece_y, &self.current_piece) {
            self.piece_x += dir;
        }
    }

    fn tick(&mut self) {
        if self.game_over { return; }

        if !self.check_collision(self.piece_x, self.piece_y + 1, &self.current_piece) {
            self.piece_y += 1;
        } else {
            self.lock_piece();
            self.clear_lines();
            self.spawn_piece();
        }
    }

    fn lock_piece(&mut self) {
        for r in 0..4 {
            for c in 0..4 {
                if self.current_piece[r][c] != 0 {
                    let board_y = self.piece_y + r as isize;
                    let board_x = self.piece_x + c as isize;
                    if board_y >= 0 && board_y < BOARD_HEIGHT as isize {
                        if board_x >= 0 && board_x < BOARD_WIDTH as isize {
                            self.board[board_y as usize][board_x as usize] = 1;
                        }
                    }
                }
            }
        }
    }

    fn clear_lines(&mut self) {
        let mut lines_cleared = 0;
        for r in (0..BOARD_HEIGHT).rev() {
            let mut is_full = true;
            for c in 0..BOARD_WIDTH {
                if self.board[r][c] == 0 {
                    is_full = false;
                    break;
                }
            }
            if is_full {
                lines_cleared += 1;
                for src_r in (1..=r).rev() {
                    self.board[src_r] = self.board[src_r - 1];
                }
                self.board[0] = [0; BOARD_WIDTH];
            }
        }
        if lines_cleared > 0 {
            self.score += lines_cleared * 100;
        }
    }

    fn render(&self) {
        // ANSI escape codes: \x1b[2J clears the terminal, \x1b[H homes the cursor to top-left
        let _ = print!("\x1b[2J\x1b[H");
        
        let _ = println!("=== ATOS::TETRIS ===");
        let _ = println!("Score: {}", self.score);
        let _ = println!("+--------------------+");

        for r in 0..BOARD_HEIGHT {
            let _ = print!("|");
            for c in 0..BOARD_WIDTH {
                let mut is_piece_block = false;
                let piece_r = r as isize - self.piece_y;
                let piece_c = c as isize - self.piece_x;
                
                if piece_r >= 0 && piece_r < 4 && piece_c >= 0 && piece_c < 4 {
                    if self.current_piece[piece_r as usize][piece_c as usize] != 0 {
                        is_piece_block = true;
                    }
                }

                if self.board[r][c] != 0 || is_piece_block {
                    let _ = print!("[]");
                } else {
                    let _ = print!("  ");
                }
            }
            let _ = println!("|");
        }

        let _ = println!("+--------------------+");
        let _ = println!("Controls: A=Left, D=Right, W=Rotate, S=Drop, Q=Quit");
    }
}

fn main() {
    let mut game = GameState::new();
    let mut game_ticks: usize = 0;

    let mut drop_frame_interval: usize = 10;

    while !game.game_over {
        game.render();

        let input = poll_char();
        if input != 0 {
            match input as char {
                'a' | 'A' => game.move_horizontal(-1),
                'd' | 'D' => game.move_horizontal(1),
                'w' | 'W' => game.rotate_piece(),
                's' | 'S' => game.tick(), // Manual drop by just tick
                'q' | 'Q' => break,
                _ => {}
            }
        }

        game_ticks += 1;
        if game_ticks >= drop_frame_interval {
            game.tick();
            game_ticks = 0;

            drop_frame_interval = match game.score {
                0..=300 => 10,
                301..=800 => 7,
                _ => 4,
            };
        }

        sleep(50);
    }

    let _ = print!("\x1b[2J\x1b[H");
    let _ = println!("=====================");
    let _ = println!("     GAME OVER!      ");
    let _ = println!("  Final Score: {}    ", game.score);
    let _ = println!("=====================");

    exit(0);
}

entry!(main);
