from pdb import set_trace as bp
import argparse
from datetime import datetime
import json
import re
import sys


def strip_ansi_codes(s):
    # This regular expression pattern matches most common ANSI escape sequences
    ansi_escape = re.compile(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])")
    return ansi_escape.sub("", s)


log_line_pattern = {
    "node": re.compile(r"^\[(?P<header>.+?)\] (?P<message>.*)"),
    "wasm": re.compile(
        r"^(?P<datetime>20.+) UTC (?P<level>[A-Z]+) (?P<source>.+:\d+) (?P<message>.*)"
    ),
}


def process_data(data_type, data, filename):
    data = strip_ansi_codes(data)
    data = re.sub(r"/home/[^/]+/[^/]+/[^/]+/(\S+)", r"\1", data)
    if data_type == "wasm":
        match = log_line_pattern[data_type].match(data)
        if match:
            date = match.group("datetime")
            if "." not in date:
                date = f"{date}.000"
            dt = datetime.strptime(date, "%Y-%m-%d %H:%M:%S.%f")
            return {
                "DateTime": dt.isoformat(),
                "Level": match.group("level"),
                "Source": match.group("source"),
                "LogMessage": match.group("message"),
                "Type": data_type,
                "Filename": filename,
            }
    elif data_type == "node":
        # [2023-06-15T18:27:57.154731Z WARN  node::managers::preprocessing::actions::generate::handlers::action_message] Failed to process protocol message: unexpected: join error
        match = log_line_pattern[data_type].match(data)
        if match:
            date, level, source = match.group("header").split()
            dt = datetime.strptime(date, "%Y-%m-%dT%H:%M:%S.%fZ")
            return {
                "DateTime": dt.isoformat(),
                "Level": level,
                "Source": source,
                "LogMessage": match.group("message"),
                "Type": data_type,
                "Filename": filename,
            }
    else:
        return None


def main():
    parser = argparse.ArgumentParser(
        description="Process logs and rewrite for QLogExplorer"
    )
    parser.add_argument(
        "-n",
        "--node",
        required=True,
        action="append",
        help="One or more node log files",
    )
    parser.add_argument(
        "-w",
        "--wasm",
        default=[],
        required=False,
        action="append",
        help="One or more wasm log files",
    )
    parser.add_argument(
        "-m",
        "--metrics",
        default=[],
        required=False,
        action="append",
        help="One or more metrics log files",
    )
    parser.add_argument(
        "-o",
        "--output",
        required=True,
        help="One JSON struct per-line of output filename",
    )

    args = parser.parse_args()

    data = []
    for filename in args.node:
        count = 0
        print(f"Processing {filename}...", file=sys.stderr, end="")
        with open(filename, "r") as infile:
            log_type = "node"
            for line in infile:
                processed_line = process_data(log_type, line.strip(), filename)
                if processed_line is not None:
                    count += 1
                    data.append(processed_line)
        print(f"   processed: {count} loglines", file=sys.stderr)

    for filename in args.wasm:
        print(f"Processing {filename}...", file=sys.stderr, end="")
        count = 0
        with open(filename, "r") as infile:
            log_type = "wasm"
            for line in infile:
                processed_line = process_data(log_type, line.strip(), filename)
                if processed_line is not None:
                    count += 1
                    data.append(processed_line)
        print(f"   processed: {count} loglines", file=sys.stderr)

    for filename in args.metrics:
        print(f"Processing {filename}...", file=sys.stderr, end="")
        count = 0
        with open(filename, "r") as infile:
            for line in infile:
                count += 1
                extra = json.loads(line)
                data.append(extra)
        print(f"   processed: {count} loglines", file=sys.stderr)

    with open(args.output, "w") as outfile:
        print(f"Writing to {args.output}...", file=sys.stderr, end="")
        count = 0
        for line in sorted(data, key=lambda x: x["DateTime"]):
            count += 1
            json.dump(line, outfile)
            outfile.write("\n")
        print(f"   processed: {count} loglines", file=sys.stderr)


if __name__ == "__main__":
    main()
