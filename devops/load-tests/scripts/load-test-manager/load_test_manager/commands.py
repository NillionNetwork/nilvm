"""
Commands supported by the load-test-manager.
"""

# pylint: disable=import-error

from datetime import datetime

import os
import yaml

from grafana_client import GrafanaApi, TokenAuth
from grafana_client.client import GrafanaException
from load_test_manager.errors import CommandError

EXPIRES_IN_DAYS = 86400 * 7
NEGATIVE_PADDING_IN_SECS = -1 * 1 * 60
SNAPSHOT_NAME = "load-test-manager-snapshot"


def grafana_snapshot(
    uuid: str, start_time_secs: int, end_time_secs: int, env_name: str
):
    """
    Takes a Grafana snapshot. Prints its URL.
    """
    token = os.getenv("GRAFANA_TOKEN")
    if token is None:
        raise CommandError("GRAFANA_TOKEN is not set")

    url = os.getenv("GRAFANA_URL")
    if url is None:
        raise CommandError("GRAFANA_URL is not set")

    grafana = GrafanaApi.from_url(credential=TokenAuth(token), url=url)

    try:
        dashboard = grafana.dashboard.get_dashboard(uuid)
    except GrafanaException as ex:
        raise CommandError(
            f"An error occurred getting Grafana dashboard '{uuid}': {ex}"
        ) from ex

    if "dashboard" not in dashboard:
        raise CommandError("Dashboard from Grafana API is missing a dashboard field")

    # Substitute env dashboard variable with correct env name.
    if "templating" not in dashboard["dashboard"]:
        raise CommandError("Grafana dashboard is missing a templating field")

    templating = dashboard["dashboard"]["templating"]

    if "list" not in templating:
        raise CommandError("Grafana dashboard is missing a templating.list field")

    templating_list = templating["list"]

    if len(templating_list) == 0:
        raise CommandError("Grafana dashboard templating.list field is empty")

    if "name" not in templating_list[0]:
        raise CommandError("Grafana dashboard is missing templating.list.0.name field")

    if templating_list[0]["name"] != "env":
        raise CommandError(
            "Grafana dashboard does not have env variable in expected place"
        )

    dashboard["dashboard"]["templating"]["list"][0]["current"] = {
        "selected": True,
        "text": env_name,
        "value": env_name,
    }

    if "time" not in dashboard["dashboard"]:
        raise CommandError(
            "Dashboard from Grafana API is missing a dashboard.time field"
        )

    if (
        "from" not in dashboard["dashboard"]["time"]
        or "to" not in dashboard["dashboard"]["time"]
    ):
        raise CommandError(
            "Dashboard from Grafana API is missing a dashboard.time.{from,to} field"
        )

    def format_secs(secs: int, padding: int = 0):
        return datetime.fromtimestamp(secs + padding).strftime("%Y-%m-%dT%H:%M:%S.%fZ")

    # Add negative padding to the start time to display a bit of what the
    # cluster metrics looked like prior to the test run.
    dashboard["dashboard"]["time"]["from"] = format_secs(
        start_time_secs, NEGATIVE_PADDING_IN_SECS
    )
    dashboard["dashboard"]["time"]["to"] = format_secs(end_time_secs)

    try:
        snapshot = grafana.snapshots.create_new_snapshot(
            dashboard=dashboard["dashboard"],
            name=SNAPSHOT_NAME,
            expires=EXPIRES_IN_DAYS,
        )
    except GrafanaException as ex:
        raise CommandError(
            f"An error occurred creating a dashboard snapshot: {ex}"
        ) from ex

    if "url" not in snapshot:
        raise CommandError("Snapshot from Grafana API is missing a url field")

    print(snapshot["url"])


def render_spec(
    spec_path: str,
    max_flow_duration: str,
    max_test_duration: str,
    operation_input_size: str,
    workers: int,
    required_starting_balance: int,
):
    """
    Renders a spec by replacing the dynamic fields.
    """
    if not os.path.isfile(spec_path):
        raise CommandError(f"Spec path file '{spec_path}' does not exist")

    with open(spec_path, "r", encoding="utf-8") as spec_path_file:
        try:
            spec_path_yaml = yaml.safe_load(spec_path_file)
        except yaml.YAMLError as ex:
            raise CommandError(
                f"An error occurred parsing spec path file as YAML: {ex}"
            ) from ex

    if max_flow_duration is not None:
        if "max_flow_duration" not in spec_path_yaml:
            raise CommandError("Spec is missing a max_flow_duration field")

        spec_path_yaml["max_flow_duration"] = max_flow_duration

    if max_test_duration is not None:
        if "max_test_duration" not in spec_path_yaml:
            raise CommandError("Spec is missing a max_test_duration field")

        spec_path_yaml["max_test_duration"] = max_test_duration

    if "operation" not in spec_path_yaml:
        raise CommandError("Spec is missing an operation field")

    if "type" not in spec_path_yaml["operation"]:
        raise CommandError("Spec is missing an operation.type field")

    operation_type = spec_path_yaml["operation"]["type"]

    if (
        operation_type in ["RetrieveValue", "StoreValues"]
        and operation_input_size is not None
    ):
        which_input = None

        if "input" in spec_path_yaml["operation"]:
            which_input = "input"
        elif "inputs" in spec_path_yaml["operation"]:
            which_input = "inputs"
        else:
            raise CommandError(
                "Spec is missing both operation.input or operation.inputs field"
            )

        if "size" not in spec_path_yaml["operation"][which_input]:
            raise CommandError(f"Spec is missing an operation.{which_input}.size field")

        spec_path_yaml["operation"][which_input]["size"] = operation_input_size

    if workers != 0:
        if "mode" not in spec_path_yaml:
            raise CommandError("Spec is missing a mode field")

        if "workers" not in spec_path_yaml["mode"]:
            raise CommandError("Spec is missing an mode.workers field")

        spec_path_yaml["mode"]["workers"] = workers

    if required_starting_balance != 0:
        spec_path_yaml["required_starting_balance"] = required_starting_balance

    with open(f"{spec_path}.rendered", "w", encoding="utf-8") as spec_path_file:
        try:
            yaml.dump(spec_path_yaml, spec_path_file)
        except yaml.YAMLError as ex:
            raise CommandError(
                f"An error occurred writing YAML to spec path rendered file: {ex}"
            ) from ex
