import json
from abc import ABC, abstractmethod
from json import JSONEncoder, JSONDecoder
import subprocess

from typing import Dict, Any

NADA_TESTS_COLLECTED = []
TWO_TO_THE_POWER_OF_64 = pow(2, 64)


class NadaTestCase:
    def __init__(self, fn, program, test_name, json_encoder, json_decoder):
        self.fn = fn
        self.program = program
        self.test_name = test_name
        self.json_encoder = json_encoder
        self.json_decoder = json_decoder
        self.file = fn.__module__

    def run(self):
        return self.fn()

    def name(self):
        return f"{self.file}:{self.test_name}"


class NadaTestJSONEncoderFirstPass(JSONEncoder):
    def default(self, o):
        if hasattr(o, 'to_json'):
            return o.to_json()
        raise TypeError(
            f"Object of type {o.__class__.__name__} is not JSON serializable, implement `to_json` method returning a serializable object in the class"
        )


class NadaTestJSONEncoder(JSONEncoder):
    def encode(self, o):
        json_data = json.dumps(o, cls=NadaTestJSONEncoderFirstPass)
        data = json.loads(json_data)
        data = NadaTestJSONEncoder.process_json_data(data)
        return super().encode(data)

    @classmethod
    def process_json_data(cls, data):
        for k, v in data.items():
            if isinstance(v, dict):
                data[k] = cls.process_json_data(v)
            if isinstance(v, list):
                for i, item in enumerate(v):
                    v[i] = cls.process_json_data(item)
            if isinstance(v, int):
                if v >= TWO_TO_THE_POWER_OF_64:
                    data[k] = str(v)
        return data


class NadaTestJSONDecoder(JSONDecoder):
    def decode(self, s) -> Any:
        data = super().decode(s)
        return NadaTestJSONDecoder.process_json_data(data)

    @classmethod
    def process_json_data(cls, data):
        for k, v in data.items():
            if isinstance(v, dict):
                data[k] = cls.process_json_data(v)
            if isinstance(v, list):
                for i, item in enumerate(v):
                    v[i] = cls.process_json_data(item)
            if isinstance(v, str):
                data[k] = int(v)
        return data


def nada_test(program: str, json_encoder=NadaTestJSONEncoder, json_decoder=NadaTestJSONDecoder):
    if not program or not isinstance(program, str):
        raise Exception("Program must be provided to @nada_test(program='...')")

    def decorator(fn_or_class):
        # if it is a class, instantiate it
        if isinstance(fn_or_class, type):
            if not issubclass(fn_or_class, NadaTest):
                raise Exception(f"Class {fn_or_class.__name__} must be a subclass of NadaTest")
            name = fn_or_class.__name__
            fn = fn_or_class()
        else:
            fn = fn_or_class
            name = fn.__name__
        NADA_TESTS_COLLECTED.append(NadaTestCase(fn, program, name, json_encoder, json_decoder))
        return fn

    return decorator


class NadaRunError(Exception):
    def __init__(self, program, stdout, stderr):
        self.program = program
        self.stdout = stdout
        self.stderr = stderr

    def __str__(self):
        return f"Error running program {self.program}:\n{self.stdout}\n{self.stderr}"


def nada_run(program: str, inputs: [Dict[str, Any]], debug, json_encoder=NadaTestJSONEncoder,
             json_decoder=NadaTestJSONDecoder) -> Dict[str, Any]:
    """
    Run a Nada program with the given inputs and return the outputs.
    :param program: program name
    :param inputs: inputs to the program
    :param debug: if True, run the program in debug mode
    :param json_encoder: encoder for serializing the inputs to json
    :param json_decoder: decoder for deserializing the outputs from json
    :return: the outputs of the program as a dictionary
    """
    inputs_json = json.dumps(inputs, cls=json_encoder)
    command = ["nada", "run-json", program]
    if debug:
        command.append("--debug")
    try:
        output = subprocess.run(
            command,
            input=inputs_json,
            capture_output=True,
            text=True,
            check=True)
    except subprocess.CalledProcessError as e:
        raise NadaRunError(program, e.stdout, e.stderr)

    except subprocess.CalledProcessError as e:
        raise e
    output = json.loads(output.stdout, cls=json_decoder)
    return output


class NadaTest(ABC):
    """
    Base class for Nada tests. Subclass this class and implement the `inputs` and `check_outputs` methods.
    """

    @abstractmethod
    def inputs(self):
        raise NotImplementedError

    @abstractmethod
    def check(self, outputs):
        raise NotImplementedError

    def __call__(self):
        outs = yield self.inputs()
        self.check(outs)
