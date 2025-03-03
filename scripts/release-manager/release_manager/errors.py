"""
Custom errors.
"""

class CommandError(Exception):
    """
    Raises when errors occur when running a command.
    """
    def __init__(self, message: str):
        super().__init__(message)

class NotFoundError(CommandError):
    """
    Raises when errors occur when running a command.
    """
