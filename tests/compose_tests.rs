use crate::compose_extras::StubLlmProvider;
use spectral::assert_that;

mod compose_extras;

assert_responses![
    filled_think_removed:
        response => "<think>This is inside think tags</think>\n## [Test Title](http://example.com)\nTest content",
        result => "## [Test Title](http://example.com)\nTest content",
    empty_think_removed:
        response => "<think>\n</think>\n## [Test Title](http://example.com)\nTest content",
        result => "## [Test Title](http://example.com)\nTest content",
];
