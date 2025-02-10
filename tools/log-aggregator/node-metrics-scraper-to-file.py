from pdb import set_trace as bp
import argparse
from datetime import datetime, timezone
import json
import http.client
import time


def scrape_metrics(targets, metrics):

    for target in targets:
        try:
            connection = http.client.HTTPConnection(target)
            connection.request("GET", "/metrics")

            response = connection.getresponse()

            if response.status == 200:
                data = response.read().decode("utf-8").split("\n")
                for line in data:
                    if not line.startswith("#") and any(
                        substring in line for substring in metrics
                    ):
                        yield line
            else:
                print(
                    f"Failed to retrieve data from {target}. Status code: {response.status_code}"
                )
        except Exception as e:
            print(f"Error occurred when retrieving data from {target}: {e}")


def main():
    parser = argparse.ArgumentParser(
        description="Gathers metrics and writes to file for use in QLogExplorer"
    )
    parser.add_argument(
        "-t",
        "--target",
        required=True,
        action="append",
        help="One or more node host:port",
    )
    parser.add_argument(
        "-m",
        "--metric",
        required=True,
        action="append",
        help="One or more prometheus metric substrings to capture",
    )
    parser.add_argument(
        "-o",
        "--output",
        required=True,
        help="One JSON struct per-line of output filename",
    )
    parser.add_argument(
        "-w",
        "--wait",
        required=False,
        default=1,
        help="How many seconds to wait between probes (default: 1)",
    )

    args = parser.parse_args()

    with open(args.output, "w") as outfile:
        while True:

            for metric in scrape_metrics(args.target, args.metric):
                json.dump(
                    {
                        "DateTime": datetime.now(timezone.utc).isoformat(),
                        "Level": "INFO",
                        "Type": "metric",
                        "Source": "metrics hook",
                        "LogMessage": metric,
                        "Filename": args.output,
                    },
                    outfile,
                )
                outfile.write("\n")
                outfile.flush()

            time.sleep(args.wait)


if __name__ == "__main__":
    main()
