import importlib
import importlib.util
import json
import os
import sys

from nada_test import NADA_TESTS_COLLECTED, nada_run, NadaRunError


def import_file(filepath):
    spec = importlib.util.spec_from_file_location(os.path.basename(filepath), filepath)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)


def import_directory(directory):
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith(".py"):
                import_file(os.path.join(root, file))


def find_test_case():
    test_name = os.environ["NADA_TEST_NAME"]
    test_case = next(test_case for test_case in NADA_TESTS_COLLECTED if test_case.name() == test_name)
    return test_case


def main():
    for arg in sys.argv[1:]:
        if os.path.isfile(arg):
            import_file(arg)
        elif os.path.isdir(arg):
            import_directory(arg)
        else:
            raise ValueError(f"Invalid path: {arg}")

    command = os.environ["NADA_TEST_COMMAND"]

    if command == "list":
        tests = [{"name": test.name(), "program": test.program} for test in NADA_TESTS_COLLECTED]
        print(json.dumps(tests))

    if command == "inputs":
        test_case = find_test_case()
        generator = test_case.run()
        inputs = next(generator)
        print(json.dumps(inputs, cls=test_case.json_encoder))

    if command == "test":
        test_case = find_test_case()
        generator = test_case.run()
        inputs = next(generator)
        debug = os.environ.get("NADA_TEST_DEBUG", "false") == "true"
        try:
            outputs = nada_run(test_case.program, inputs, debug, test_case.json_encoder, test_case.json_decoder)
        except NadaRunError as e:
            print(e)
            sys.exit(1)
        try:
            generator.send(outputs)
        except StopIteration:
            pass
