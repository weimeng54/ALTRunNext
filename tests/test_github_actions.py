import sys
import unittest
from pathlib import Path
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import github_actions

class BranchGuardTests(unittest.TestCase):
    def test_sync_rejects_an_unrelated_current_branch_before_network(self):
        def git(args, **kwargs):
            if args[1:]==['symbolic-ref','--quiet','--short','HEAD']:
                return b'other-branch\n'
            return b'abc123\n'
        with patch.object(github_actions.subprocess,'check_output',side_effect=git), patch.object(github_actions,'request',side_effect=RuntimeError('network must not run')) as network:
            with self.assertRaisesRegex(RuntimeError,'wrong branch'):
                github_actions.sync()
            network.assert_not_called()

if __name__=='__main__':unittest.main()
