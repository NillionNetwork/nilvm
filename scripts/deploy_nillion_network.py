#!/usr/bin/env python3

"""
A program which triggers a deploy of nillion-network waits for completion.
"""

# Standard imports.
import argparse
import datetime
import sys
import time

# 3rd party imports.
# pylint: disable=import-error
import jenkins

COMPLETED_BUILD_SLEEP_SECS = 30
QUEUED_BUILD_SLEEP_SECS = 10
TIMEOUT_TIME_IN_MINS = 30


class CancelledError(Exception):
    """
    Raised when Jenkins job is cancelled amid waiting for it.
    """


class TimedOutError(Exception):
    """
    Raised when timeout error occurs.
    """


def wait_for(operation, predicate, wait_message, wait_for_secs):
    """
    Performs operation and waits for predicate to return true.
    """
    start_time = datetime.datetime.now()
    current_time = start_time
    timeout_time = start_time + datetime.timedelta(minutes=TIMEOUT_TIME_IN_MINS)

    while current_time < timeout_time:
        return_value = operation()

        if predicate(return_value):
            return return_value

        # If we're going to sleep only to then immediately timeout, then timeout now.
        if (current_time + datetime.timedelta(seconds=wait_for_secs)) > timeout_time:
            raise TimedOutError()

        print(wait_message)

        time.sleep(wait_for_secs)

        current_time = datetime.datetime.now()

    raise TimedOutError()


def wait_for_scheduled_build(jenkins_client, queue_item_number):
    """
    Waits for build info of scheduled build to be returned.
    """

    def queue_item_operation():
        # pylint: disable=broad-exception-caught
        try:
            return jenkins_client.get_queue_item(queue_item_number)
        except jenkins.NotFoundException:
            pass
        except Exception as ex:
            print(f"Handled unexpected exception while waiting for build to schedule: {ex}.")

        return None

    def queue_item_predicate(queue_item):
        if "cancelled" in queue_item and queue_item["cancelled"] is True:
            raise CancelledError()

        # pylint: disable=line-too-long
        return queue_item is not None and \
            "executable" in queue_item and \
            queue_item["executable"] is not None and \
            "number" in queue_item["executable"]

    return wait_for(
        queue_item_operation,
        queue_item_predicate,
        # pylint: disable=line-too-long
        f"Build info for queue item {queue_item_number} not ready. Sleeping {QUEUED_BUILD_SLEEP_SECS} seconds before trying again.",
        wait_for_secs=QUEUED_BUILD_SLEEP_SECS,
    )


def wait_for_completed_build(nillion_network_environment, jenkins_client, build_number):
    """
    Waits for scheduled build to complete.
    """

    def completed_build_operation():
        # pylint: disable=broad-exception-caught
        try:
            return jenkins_client.get_build_info(
                f"nillion-network/deploy-{nillion_network_environment}",
                build_number,
            )
        except jenkins.NotFoundException:
            pass
        except Exception as ex:
            print(f"Handled unexpected exception while waiting for build to complete: {ex}.")

        return None

    def completed_build_predicate(build_info):
        return build_info is not None and \
            build_info["number"] == build_number and \
            "result" in build_info and \
            build_info["result"] is not None

    return wait_for(
        completed_build_operation,
        completed_build_predicate,
        # pylint: disable=line-too-long
        f"Build {build_number} has not yet completed. Sleeping {COMPLETED_BUILD_SLEEP_SECS} seconds before trying again.",
        wait_for_secs=COMPLETED_BUILD_SLEEP_SECS,
    )


def main():
    """
    Runs the program.
    """
    # Setup arg parser and parse args.
    arg_parser = argparse.ArgumentParser(description="Triggers a deploy of nillion-network")

    # Add arguments.
    arg_parser.add_argument("jenkins_url", type=str, help="Jenkins API URL")
    arg_parser.add_argument("jenkins_username", type=str, help="Jenkins API user")
    arg_parser.add_argument("jenkins_password", type=str, help="Jenkins API password")
    arg_parser.add_argument("devops_repo_commit", type=str, help="Parameter to feed into job")
    arg_parser.add_argument("new_release_version", type=str, help="Parameter to feed into job")
    arg_parser.add_argument("nillion_network_environment", type=str, help="Environment to deploy")
    arg_parser.add_argument("run_functional_tests", type=bool, default=False, help="Run functional tests after deploy")

    args = arg_parser.parse_args()

    # Trigger build.
    jenkins_client = jenkins.Jenkins(
        url=args.jenkins_url,
        username=args.jenkins_username,
        password=args.jenkins_password,
    )

    queue_item_number = jenkins_client.build_job(
        f"nillion-network/deploy-{args.nillion_network_environment}",
        parameters={
            "BRANCH": args.devops_repo_commit,
            "ENVIRONMENT": args.nillion_network_environment,
            "RELEASE_VERSION": args.new_release_version,
            "RUN_FUNCTIONAL_TESTS": args.run_functional_tests,
        },
    )
    print(
        "Job Link: " +
        f"{args.jenkins_url}/job/nillion-network/job/deploy-{args.nillion_network_environment}/{queue_item_number}"
    )
    # Wait for build to be scheduled.
    try:
        scheduled_build = wait_for_scheduled_build(jenkins_client, queue_item_number)
    except CancelledError as ex:
        # pylint: disable=broad-exception-raised
        raise Exception(
            f"Jenkins queue item '{queue_item_number}' cancelled amid waiting for it"
        ) from ex
    except TimedOutError as ex:
        # pylint: disable=broad-exception-raised
        raise Exception(
            f"Timed-out after {TIMEOUT_TIME_IN_MINS} minutes waiting for queue item with"
            f" queue item number {queue_item_number}'."
        ) from ex

    build_number = scheduled_build["executable"]["number"]

    # Wait for build to complete.
    try:
        completed_build = wait_for_completed_build(
            args.nillion_network_environment,
            jenkins_client,
            build_number,
        )
    except TimedOutError as ex:
        # pylint: disable=broad-exception-raised
        raise Exception(
            # pylint: disable=line-too-long
            f"Timed-out after {TIMEOUT_TIME_IN_MINS} minutes waiting for build '{build_number}' to complete'."
        ) from ex

    build_result = completed_build["result"]

    print(f"Build '{build_number}' has completed with status '{build_result}'")

    if build_result != "SUCCESS":
        sys.exit(1)


if __name__ == "__main__":
    main()
