use super::runner::RunResult;
use std::time::Duration;

pub struct InterpRunner {
    bodies: Vec<std::sync::Arc<glyim_mir::Body>>,
}

impl InterpRunner {
    pub fn new(bodies: Vec<std::sync::Arc<glyim_mir::Body>>) -> Self {
        Self { bodies }
    }

    pub fn run(self, timeout: Duration) -> RunResult {
        let start = std::time::Instant::now();
        let timeout_secs = timeout.as_secs();

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let output = interpret_bodies(&self.bodies);
            let _ = tx.send(output);
        });

        match rx.recv_timeout(timeout) {
            Ok(output) => {
                let duration = start.elapsed();
                RunResult {
                    exit_code: Some(output.exit_code),
                    stdout: output.stdout,
                    stderr: output.stderr,
                    timed_out: false,
                    duration,
                }
            }
            Err(_) => {
                let duration = start.elapsed();
                RunResult {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: format!("interpreter timed out after {}s", timeout_secs),
                    timed_out: true,
                    duration,
                }
            }
        }
    }
}

struct InterpOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn interpret_bodies(bodies: &[std::sync::Arc<glyim_mir::Body>]) -> InterpOutput {
    let stdout = String::new();
    let mut stderr = String::new();

    if bodies.is_empty() {
        return InterpOutput { exit_code: 0, stdout, stderr };
    }

    let body = &bodies[0];
    let mut exit_code = 0;

    for (_idx, block) in body.basic_blocks.iter_enumerated() {
        match &block.terminator.kind {
            glyim_mir::TerminatorKind::Return => break,
            glyim_mir::TerminatorKind::Unreachable => {
                stderr.push_str("runtime error: reached unreachable\n");
                exit_code = 101;
                break;
            }
            _ => {}
        }
    }

    InterpOutput { exit_code, stdout, stderr }
}
