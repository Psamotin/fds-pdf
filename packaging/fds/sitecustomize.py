"""FDS Windows child-process policy, activated only by the desktop OCR adapter.

Python's CREATE_NO_WINDOW is not inherited by children. OCRmyPDF and Python
multiprocessing both launch through _winapi.CreateProcess. The pinned Python
3.13 runtime applies this single process-creation flag without changing OCR code.
"""
import os
import sys

if sys.platform == 'win32' and os.environ.get('FDS_OCR_HIDE_CHILDREN') == '1':
    import _winapi

    if not getattr(_winapi, '_fds_hidden_children', False):
        _original_create_process = _winapi.CreateProcess

        def _create_hidden_process(application_name, command_line, process_attributes,
                                   thread_attributes, inherit_handles, creation_flags,
                                   environment, current_directory, startup_info):
            return _original_create_process(
                application_name, command_line, process_attributes, thread_attributes,
                inherit_handles, creation_flags | 0x08000000, environment,
                current_directory, startup_info,
            )

        _winapi.CreateProcess = _create_hidden_process
        _winapi._fds_hidden_children = True
