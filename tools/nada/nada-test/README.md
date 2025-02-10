# nada-test Testing Framework for `nada`

nada-test is a testing framework for `nada` programs. It allows you to write tests for your `nada` programs in a simple
and easy way.
nada-test are python functions that you write so they are more flexible than yaml test files. You can use python code to
compose the inputs, run the program and then check the outputs are valid.

To use nada-test you have to install it with pip:

```bash
pip install nada-test
or 
pip install path/to/nada-test
```

### How to write a test

nada-test are python code that you write in your test file. You have to decorate your test functions with
the `@nada_test` decorator.

Here an example:

```python
from nada_test import nada_test, NadaTest


# Functional style test
@nada_test(program="main")
def my_test():
    a = 1
    b = 2
    outputs = yield {"A": a, "B": b}
    assert outputs["my_output"] == a + b


# Class style test
@nada_test(program="main")
class Test(NadaTest):
    def inputs(self):
        return {"A": 1, "B": 2}

    def check(self, outputs):
        assert outputs["my_output"] == 3
```

### Configure nada-test in `nada-project.toml`

nada-test has its own runner, to use it add the following to
the `nada-project.toml` file:

```toml
[test_framework.nada-test]
command = "nada-test ./tests"
```

that will configure `nada test` to use nada-test as test framework.

then after writing your tests in the `./tests` directory you can run the tests with the following command:

```bash
nada test 
```