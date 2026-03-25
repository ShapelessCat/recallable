.DEFAULT_GOAL := help

CARGO ?= cargo
TEST_ARGS ?= --workspace
CARGO_LLVM_COV ?= cargo llvm-cov
COVERAGE_ARGS ?= --workspace
COVERAGE_DIR ?= target/llvm-cov
COVERAGE_HTML_INDEX := $(COVERAGE_DIR)/html/index.html
BROWSER_OPEN ?= open

.PHONY: help test test-default test-no-default test-impl-from test-all-features coverage coverage-open

help:
	@printf '%s\n' \
		'Available targets:' \
		'  make test               Run the full workspace test feature matrix' \
		'  make test-default       Run tests with default features' \
		'  make test-no-default    Run tests with no default features' \
		'  make test-impl-from     Run tests with impl_from enabled' \
		'  make test-all-features  Run tests with all features enabled' \
		'  make coverage           Generate merged llvm-cov HTML and JSON reports' \
		'  make coverage-open      Generate coverage reports and open the HTML report'

test: test-default test-no-default test-impl-from test-all-features

test-default:
	$(CARGO) test $(TEST_ARGS)

test-no-default:
	$(CARGO) test $(TEST_ARGS) --no-default-features

test-impl-from:
	$(CARGO) test $(TEST_ARGS) --features impl_from

test-all-features:
	$(CARGO) test $(TEST_ARGS) --all-features

coverage:
	$(CARGO_LLVM_COV) clean $(COVERAGE_ARGS)
	$(CARGO_LLVM_COV) $(COVERAGE_ARGS) --no-default-features --tests --no-report
	$(CARGO_LLVM_COV) $(COVERAGE_ARGS) --tests --no-report
	$(CARGO_LLVM_COV) $(COVERAGE_ARGS) --features impl_from --tests --no-report
	$(CARGO_LLVM_COV) $(COVERAGE_ARGS) --all-features --tests --no-report
	$(CARGO_LLVM_COV) report --html --output-dir $(COVERAGE_DIR)
	$(CARGO_LLVM_COV) report --json --summary-only --output-path $(COVERAGE_DIR)/summary.json

coverage-open: coverage
	$(BROWSER_OPEN) $(COVERAGE_HTML_INDEX)
