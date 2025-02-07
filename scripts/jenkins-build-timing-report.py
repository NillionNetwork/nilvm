from pdb import set_trace as bp
import jenkins, os, json, requests, sys, argparse
from waterbear import Bear

JENKINS_HOME = "https://jenkins-internal.nilogy.xyz"


def print_duration_from_result(data):
    for gitchangeset in data.changeSets:
        for item in gitchangeset["items"]:
            print(f"{data.number}\t{data.duration}\t{item['msg']}")
            return


if __name__ == "__main__":

    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--jobname",
        default="nillion/feature%252Fjenkins-pipeline-improvements",
        help="Jenkins job name; note this may be encoded",
    )
    parser.add_argument(
        "--count",
        type=int,
        default=30,
        help="how many builds to fetch from most recent; default is 30",
    )
    args = parser.parse_args()

    server = jenkins.Jenkins(
        JENKINS_HOME,
        username=os.environ["JENKINS_USER"],
        password=os.environ["JENKINS_TOKEN"],
    )
    user = server.get_whoami()
    version = server.get_version()

    print("Hello %s from Jenkins %s" % (user["fullName"], version), file=sys.stderr)

    last_build_number = server.get_job_info(args.jobname)["lastCompletedBuild"][
        "number"
    ]

    print("build number\tduration\tcommit message")
    for build in range(last_build_number - args.count, last_build_number):
        try:
            build_info = server.get_build_info(args.jobname, build)
            data = Bear(**build_info)
            print_duration_from_result(data)
        except jenkins.JenkinsException as e:
            print(e)
            pass
        except Exception as e:
            raise e
