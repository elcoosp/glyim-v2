use super::config::TestMode;
use super::collector::TestCollector;
use super::executor::TestExecutor;
use super::reporter::TestReporter;
use crate::error::TestDiscoveryError;
use std::path::PathBuf;
use std::time::Duration;

pub struct TestRunner {
    root: PathBuf,
    mode_override: Option<TestMode>,
    parallel: bool,
    filter: Option<String>,
    timeout: Duration,
    max_concurrent: usize,
    use_pipeline: bool,
}

impl TestRunner {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            mode_override: None,
            parallel: true,
            filter: None,
            timeout: Duration::from_secs(60),
            max_concurrent: 8,
            use_pipeline: true,
        }
    }
    pub fn mode(mut self, mode: TestMode) -> Self { self.mode_override = Some(mode); self }
    pub fn parallel(mut self, yes: bool) -> Self { self.parallel = yes; self }
    pub fn filter(mut self, f: impl Into<String>) -> Self { self.filter = Some(f.into()); self }
    pub fn timeout(mut self, d: Duration) -> Self { self.timeout = d; self }
    pub fn max_concurrent(mut self, n: usize) -> Self { self.max_concurrent = n; self }
    pub fn frontend_only(mut self) -> Self { self.use_pipeline = false; self }

    pub fn build(self) -> Result<TestPlan, TestDiscoveryError> {
        let collector = TestCollector::new(&self.root);
        let tests = collector.collect(self.filter.as_deref(), self.mode_override)?;
        Ok(TestPlan {
            tests,
            parallel: self.parallel,
            default_timeout: self.timeout,
            max_concurrent: self.max_concurrent,
            use_pipeline: self.use_pipeline,
            bless: std::env::var("GLYIM_BLESS").is_ok(),
            verbose: std::env::var("GLYIM_TEST_SHOW_OUTPUT").is_ok(),
        })
    }
}

pub struct TestPlan {
    pub tests: Vec<std::sync::Arc<super::collector::DiscoveredTest>>,
    pub parallel: bool,
    pub default_timeout: Duration,
    pub max_concurrent: usize,
    pub use_pipeline: bool,
    pub bless: bool,
    pub verbose: bool,
}

pub struct ExecutionResult {
    pub results: Vec<super::executor::TestResult>,
    pub summary: TestSummary,
}

impl TestPlan {
    pub fn execute(self) -> ExecutionResult {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        if self.tests.is_empty() {
            eprintln!("no test files found");
            return ExecutionResult { results: Vec::new(), summary: TestSummary::default() };
        }

        eprintln!("running {} tests", self.tests.len());

        let executor = TestExecutor::new(
            self.default_timeout, self.bless, self.verbose,
            self.max_concurrent, self.use_pipeline,
        );
        let results = if self.parallel {
            executor.run_parallel(&self.tests)
        } else {
            executor.run_sequential(&self.tests)
        };

        let reporter = TestReporter::new(self.verbose);
        let summary = reporter.report(&results);

        ExecutionResult { results, summary }
    }

    pub fn run(self) {
        let result = self.execute();
        if result.summary.failed > 0 {
            let failed_names: Vec<_> = result.results.iter()
                .filter(|r| matches!(r.outcome, super::executor::TestOutcome::Failed { .. }))
                .map(|r| format!("{} [{}]", r.test.name, r.revision))
                .collect();
            panic!(
                "{} tests FAILED: {}\n\
                 Run with GLYIM_TEST_SHOW_OUTPUT=1 for details.",
                result.summary.failed,
                failed_names.join(", ")
            );
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub ignored: usize,
}
