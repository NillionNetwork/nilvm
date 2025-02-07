"""
Class file for S3Artifact.
"""

# pylint: disable=import-error

import boto3

from release_manager.errors import CommandError, NotFoundError

# pylint: disable=too-few-public-methods
class S3Artifact:
    """
    Interface for working with release artifacts on S3.
    """
    def __init__(self, bucket_name: str, release_version: str):
        self.bucket_name = bucket_name
        self.release_version = release_version

    def check(self):
        """
        Checks if a release exists in the bucket and returns its objects.
        """
        s3_client = boto3.client("s3")

        try:
            objects = s3_client.list_objects(
                Bucket=self.bucket_name,
                Prefix=f"{self.release_version}/"
            )
        except s3_client.exceptions.NoSuchBucket as ex:
            raise CommandError(
                f"An error occurred listing objects for release '{self.release_version}' "
                f"in bucket '{self.bucket_name}'"
            ) from ex

        if "Contents" not in objects or len(objects["Contents"]) == 0:
            raise NotFoundError(
                f"Release '{self.release_version}' not found or has no files in bucket '{self.bucket_name}'"
            )

        return objects["Contents"]

    def copy(self, to: str):
        """
        Copies a release to a new location within the same bucket.
        """
        contents = self.check()

        # Release folder exists. Copy its contents to the new location.
        s3_resource = boto3.resource("s3")
        bucket = s3_resource.Bucket(self.bucket_name)

        for obj in contents:
            source = {
                'Bucket': self.bucket_name,
                'Key': obj['Key']
            }
            dest = obj['Key'].replace(self.release_version, to, 1)
            bucket.copy(source, dest)

    def delete(self):
        """
        Deletes a release from its bucket.
        """
        contents = self.check()

        # Release folder exists. Delete its contents.
        s3_resource = boto3.resource("s3")

        example_releases = s3_resource.Bucket(self.bucket_name)
        example_releases.objects.filter(Prefix=f"{self.release_version}/").delete()

    def sync(self, to: str):
        """
        Syncs a release to a new location within the same bucket.
        Mimics the AWS CLI 's3 sync' operation.
        """
        s3_client = boto3.client("s3")
        s3_resource = boto3.resource("s3")
        bucket = s3_resource.Bucket(self.bucket_name)

        # Get objects in source and destination
        source_objects = self.check()
        dest_objects = s3_client.list_objects_v2(
            Bucket=self.bucket_name,
            Prefix=f"{to}/"
        ).get('Contents', [])

        source_keys = {obj['Key'].replace(self.release_version, to, 1) for obj in source_objects}
        dest_keys = {obj['Key'] for obj in dest_objects}

        self.copy(to)

        # Delete objects in destination that aren't in source
        objects_to_delete = dest_keys - source_keys
        if objects_to_delete:
            delete_objects = [{'Key': key} for key in objects_to_delete]
            bucket.delete_objects(
                Delete={
                    'Objects': delete_objects,
                    'Quiet': True
                }
            )
