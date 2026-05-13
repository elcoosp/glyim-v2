use super::collector::DiscoveredTest;
use super::compiler::{FrontendOnlyCompiler, PipelineCompiler, TestCompiler};
use super::strategy;
use crate::error::{FailureReason, TimeoutError};
use glyim_diag::GlyimDiagnostic;
use glyim_span::FileId;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

static NEXT_FILE_ID: AtomicU32 = AtomicU32::new(1);

fn next_file_id() -> FileId {
    FileId::from_raw(NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone, Debug)]
pub enum TestOutcome {
    Passed,
    Failed { reason: FailureReason },
    Ignored,
}

#[derive(Clone, Debug)]
pub struct TestResult {
    pub test: Arc<DiscoveredTest>,
    pub revision: String,
    pub outcome: TestOutcome,
    pub duration: Duration,
    pub diagnostics: Vec<GlyimDiagnostic>,
}

#[allow(dead_code)]
pub struct TestExecutor {
    default_timeout: Duration,
    bless: bool,
    verbose: bool,
    max_concurrent: usize,
    target_triple: String,
    compiler: Arc<dyn TestCompiler>,
}

impl TestExecutor {
    pub fn new(
        default_timeout: Duration,
        bless: bool,
        verbose: bool,
        max_concurrent: usize,
        use_pipeline: bool,
    ) -> Self {
        let compiler: Arc<dyn TestCompiler> = if use_pipeline {
            Arc::new(PipelineCompiler::new(Arc::new(
                crate::mock::MockCodegen::new(),
            )))
        } else {
            Arc::new(FrontendOnlyCompiler)
        };

        Self {
            default_timeout,
            bless,
            verbose,
            max_concurrent,
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            compiler,
        }
    }

    pub fn with_target_triple(mut self, triple: impl Into<String>) -> Self {
        self.target_triple = triple.into();
        self
    }

    pub fn with_compiler(mut self, compiler: Arc<dyn TestCompiler>) -> Self {
        self.compiler = compiler;
        self
    }

    pub fn run_sequential(&self, tests: &[Arc<DiscoveredTest>]) -> Vec<TestResult> {
        tests
            .iter()
            .flat_map(|t| {
                let revs: Vec<String> = t.revisions.clone();
                revs.into_iter().map(|r| {
                    execute_test(
                        Arc::clone(t),
                        r,
                        Arc::clone(&self.compiler),
                        self.bless,
                        self.target_triple.clone(),
                    )
                })
            })
            .collect()
    }

    pub fn run_parallel(&self, tests: &[Arc<DiscoveredTest>]) -> Vec<TestResult> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_concurrent)
            .build()
            .unwrap();
        let tests_c: Vec<Arc<DiscoveredTest>> = tests.to_vec();
        let compiler = Arc::clone(&self.compiler);
        let bless = self.bless;
        let target_triple = self.target_triple.clone();
        pool.install(move || {
            tests_c
                .par_iter()
                .flat_map(|t| {
                    let revs: Vec<String> = t.revisions.clone();
                    let comp = Arc::clone(&compiler);
                    let tgt = target_triple.clone();
                    revs.into_iter()
                        .map(move |r| {
                            execute_test(Arc::clone(t), r, Arc::clone(&comp), bless, tgt.clone())
                        })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
    }
}

fn execute_test(
    test: Arc<DiscoveredTest>,
    revision: String,
    compiler: Arc<dyn TestCompiler>,
    bless: bool,
    target_triple: String,
) -> TestResult {
    let span = tracing::info_span!("test", name = %test.name, revision = %revision);
    let _enter = span.enter();
    let start = std::time::Instant::now();

    if test.config.ignore {
        return TestResult {
            test,
            revision,
            outcome: TestOutcome::Ignored,
            duration: start.elapsed(),
            diagnostics: Vec::new(),
        };
    }

    if test.config.needs_llvm && std::env::var("GLYIM_LLVM").is_err() {
        return TestResult {
            test,
            revision,
            outcome: TestOutcome::Ignored,
            duration: start.elapsed(),
            diagnostics: Vec::new(),
        };
    }

    if let Some(ref target) = test.config.only_target
        && target != &target_triple {
            return TestResult {
                test,
                revision,
                outcome: TestOutcome::Ignored,
                duration: start.elapsed(),
                diagnostics: Vec::new(),
            };
        }

    let timeout = Duration::from_secs(test.config.timeout_secs);
    let test_clone = Arc::clone(&test);
    let revision_clone = revision.clone();

    let result = run_with_timeout(timeout, move || {
        execute_inner(&test_clone, &revision_clone, &*compiler, bless)
    });

    let (outcome, diagnostics) = match result {
        Ok((outcome, diags)) => (outcome, diags),
        Err(TimeoutError { timeout_secs }) => (
            TestOutcome::Failed {
                reason: FailureReason::TimeoutExceeded { timeout_secs },
            },
            Vec::new(),
        ),
    };

    let duration = start.elapsed();
    tracing::info!(duration_ms = duration.as_millis(), "done");

    TestResult {
        test,
        revision,
        outcome,
        duration,
        diagnostics,
    }
}

fn execute_inner(
    test: &Arc<DiscoveredTest>,
    revision: &str,
    compiler: &dyn TestCompiler,
    bless: bool,
) -> (TestOutcome, Vec<GlyimDiagnostic>) {
    let mut flags = test.config.compile_flags.clone();
    if let Some(rev_flags) = test.config.revision_compile_flags.get(revision) {
        flags.extend(rev_flags.iter().cloned());
    }

    let file_id = next_file_id();

    let compile_span = tracing::info_span!("compile", file_id = file_id.to_raw());
    let output = compile_span.in_scope(|| compiler.compile(&test.source, file_id, &flags));

    let run_timeout = std::time::Duration::from_secs(test.config.timeout_secs);

    let outcome = match test.config.mode {
        super::config::TestMode::CompilePass => {
            strategy::CompilePassStrategy.evaluate(&output.diagnostics, &test.source)
        }
        super::config::TestMode::CompileFail => {
            strategy::CompileFailStrategy.evaluate(
                &output.diagnostics,
                &test.source,
                &test.config.error_patterns,
            )
        }
        super::config::TestMode::Ui => {
            strategy::UiTestStrategy.evaluate(&output, &test.source, &test.path, bless)
        }
        super::config::TestMode::RunPass => {
            strategy::RunPassStrategy.evaluate(
                &output,
                &test.source,
                None,
                &test.config,
                run_timeout,
            )
        }
        super::config::TestMode::RunFail => {
            strategy::RunFailStrategy.evaluate(
                &output,
                &test.source,
                None,
                &test.config,
                run_timeout,
            )
        }
    };

    (outcome, output.diagnostics)
}

fn run_with_timeout<F, R>(timeout: Duration, f: F) -> Result<R, TimeoutError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    let timeout_secs = timeout.as_secs();

    std::thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(result) => Ok(result),
        Err(_) => Err(TimeoutError { timeout_secs }),
    }
}
