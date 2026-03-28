.DEFAULT_GOAL := help

CARGO ?= cargo
TEST_ARGS ?= --workspace
EXAMPLE_ARGS ?= --package recallable
CARGO_LLVM_COV ?= cargo llvm-cov
COVERAGE_ARGS ?= --workspace
COVERAGE_DIR ?= target/llvm-cov
COVERAGE_HTML_INDEX := $(COVERAGE_DIR)/html/index.html
BROWSER_OPEN ?= open

.PHONY: help regression test validate-examples \
	test-default test-no-default test-impl-from test-all-features \
	examples-default examples-no-default examples-impl-from examples-all-features \
	coverage coverage-open

help:
	@printf '%s\n' \
		'Available targets:' \
		'  make regression         Run the full workspace test and example matrix' \
		'  make test               Run the original workspace test feature matrix' \
		'  make validate-examples  Run the example validation matrix' \
		'  make test-default       Run tests with default features' \
		'  make examples-default   Run default-feature examples' \
		'  make test-no-default    Run tests with no default features' \
		'  make examples-no-default Run examples with no default features' \
		'  make test-impl-from     Run tests with impl_from enabled' \
		'  make examples-impl-from Run impl_from examples with no default features' \
		'  make test-all-features  Run tests with all features enabled' \
		'  make examples-all-features Check examples with all features enabled' \
		'  make coverage           Generate merged llvm-cov HTML and JSON reports' \
		'  make coverage-open      Generate coverage reports and open the HTML report'

regression: test validate-examples

test: \
	test-default \
	test-no-default \
	test-impl-from \
	test-all-features

validate-examples: \
	examples-default \
	examples-no-default \
	examples-impl-from \
	examples-all-features

test-default:
	$(CARGO) test $(TEST_ARGS)

examples-default:
	$(CARGO) run $(EXAMPLE_ARGS) --example basic_model
	$(CARGO) run $(EXAMPLE_ARGS) --example nested_generic
	$(CARGO) run $(EXAMPLE_ARGS) --example postcard_roundtrip

test-no-default:
	$(CARGO) test $(TEST_ARGS) --no-default-features

examples-no-default:
	$(CARGO) run $(EXAMPLE_ARGS) --no-default-features --example manual_no_serde

test-impl-from:
	$(CARGO) test $(TEST_ARGS) --features impl_from

examples-impl-from:
	$(CARGO) run $(EXAMPLE_ARGS) --no-default-features --features impl_from --example impl_from_roundtrip

test-all-features:
	$(CARGO) test $(TEST_ARGS) --all-features

examples-all-features:
	$(CARGO) check $(EXAMPLE_ARGS) --examples --all-features

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
