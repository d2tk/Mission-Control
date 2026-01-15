#!/usr/bin/env python3
"""
Project Sentry - Automated Workspace Audit System
Monitors /home/a2/Desktop/gem for changes and generates daily reports
"""

import os
import json
import subprocess
import shutil
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Tuple

WORKSPACE = Path("/home/a2/Desktop/gem")
STATE_FILE = Path("/home/a2/Desktop/gem/opb/backend/.sentry_state.json")
IGNORE_PATTERNS = {'.git', '__pycache__', 'node_modules', '.venv', 'browser_data'}

class SentryAudit:
    def __init__(self):
        self.workspace = WORKSPACE
        self.state_file = STATE_FILE
        self.current_snapshot = {}
        self.previous_snapshot = {}
        self.report = []
        
    def load_previous_state(self):
        """Load the last snapshot from disk"""
        if self.state_file.exists():
            try:
                with open(self.state_file, 'r') as f:
                    data = json.load(f)
                    self.previous_snapshot = data.get('files', {})
                    return data.get('timestamp')
            except: pass
        return None
    
    def save_current_state(self):
        """Persist current snapshot for next run"""
        state = {
            'timestamp': datetime.now().isoformat(),
            'files': self.current_snapshot
        }
        with open(self.state_file, 'w') as f:
            json.dump(state, f, indent=2)
    
    def scan_filesystem(self):
        """Walk workspace and capture file metadata"""
        for root, dirs, files in os.walk(self.workspace):
            # Filter ignored directories
            dirs[:] = [d for d in dirs if d not in IGNORE_PATTERNS]
            
            for file in files:
                filepath = Path(root) / file
                try:
                    stat = filepath.stat()
                    rel_path = str(filepath.relative_to(self.workspace))
                    self.current_snapshot[rel_path] = {
                        'size': stat.st_size,
                        'mtime': int(stat.st_mtime)
                    }
                except: pass
    
    def analyze_changes(self) -> Dict:
        """Compare current vs previous snapshot"""
        current_files = set(self.current_snapshot.keys())
        previous_files = set(self.previous_snapshot.keys())
        
        created = current_files - previous_files
        deleted = previous_files - current_files
        modified = []
        
        for path in current_files & previous_files:
            if self.current_snapshot[path] != self.previous_snapshot.get(path):
                modified.append(path)
        
        # Calculate size changes
        total_delta = 0
        for path in created:
            total_delta += self.current_snapshot[path]['size']
        for path in deleted:
            total_delta -= self.previous_snapshot[path]['size']
        
        return {
            'created': list(created),
            'modified': modified,
            'deleted': list(deleted),
            'total_delta_mb': total_delta / (1024 * 1024),
            'churn': len(created) + len(modified) + len(deleted)
        }
    
    def get_git_status(self) -> Dict:
        """Query git repository status"""
        if not (self.workspace / '.git').exists():
            return {'available': False}
        
        try:
            # Current branch
            branch = subprocess.check_output(
                ['git', 'branch', '--show-current'],
                cwd=self.workspace, text=True
            ).strip()
            
            # Porcelain status
            status = subprocess.check_output(
                ['git', 'status', '--porcelain=v1'],
                cwd=self.workspace, text=True
            ).strip()
            
            lines = status.split('\n') if status else []
            modified = sum(1 for l in lines if l.startswith(' M'))
            staged = sum(1 for l in lines if l.startswith('M '))
            untracked = sum(1 for l in lines if l.startswith('??'))
            
            # Last commit
            try:
                last_commit = subprocess.check_output(
                    ['git', 'log', '-1', '--format=%h|%cr'],
                    cwd=self.workspace, text=True
                ).strip().split('|')
                commit_hash = last_commit[0]
                commit_age = last_commit[1]
            except:
                commit_hash = 'N/A'
                commit_age = 'N/A'
            
            return {
                'available': True,
                'branch': branch,
                'clean': len(lines) == 0,
                'modified': modified,
                'staged': staged,
                'untracked': untracked,
                'last_commit': commit_hash,
                'commit_age': commit_age
            }
        except:
            return {'available': False, 'error': 'Failed to query git'}
    
    def get_disk_usage(self) -> Dict:
        """Check disk and workspace usage"""
        usage = shutil.disk_usage(self.workspace)
        
        # Workspace size
        try:
            result = subprocess.check_output(
                ['du', '-sb', str(self.workspace)],
                text=True
            )
            workspace_bytes = int(result.split()[0])
        except:
            workspace_bytes = 0
        
        return {
            'total_gb': usage.total / (1024**3),
            'used_gb': usage.used / (1024**3),
            'free_gb': usage.free / (1024**3),
            'usage_percent': (usage.used / usage.total) * 100,
            'workspace_gb': workspace_bytes / (1024**3)
        }
    
    def generate_report(self, changes: Dict, git: Dict, disk: Dict) -> str:
        """Generate markdown report"""
        lines = []
        lines.append("=== PROJECT SENTRY DAILY REPORT ===")
        lines.append(f"Date: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
        lines.append(f"Workspace: {self.workspace}")
        lines.append("")
        
        # Summary
        lines.append("[SUMMARY]")
        churn_level = "HIGH" if changes['churn'] > 20 else "MEDIUM" if changes['churn'] > 5 else "LOW"
        lines.append(f"File changes: +{len(changes['created'])} ~{len(changes['modified'])} -{len(changes['deleted'])}   (Churn: {churn_level})")
        
        if git['available']:
            git_status = "CLEAN" if git['clean'] else "DIRTY"
            lines.append(f"Git status: {git_status} (branch: {git['branch']})")
        
        disk_status = "WARNING" if disk['usage_percent'] > 80 else "OK"
        lines.append(f"Disk usage: {disk['usage_percent']:.0f}% ({disk_status})")
        lines.append("")
        
        # Filesystem
        lines.append("[FILESYSTEM]")
        lines.append(f"Created: {len(changes['created'])}")
        lines.append(f"Modified: {len(changes['modified'])}")
        lines.append(f"Deleted: {len(changes['deleted'])}")
        lines.append(f"Size delta: {changes['total_delta_mb']:+.1f} MB")
        
        if changes['created'][:3]:
            lines.append("Top created:")
            for f in changes['created'][:3]:
                lines.append(f"  - {f}")
        lines.append("")
        
        # Git
        if git['available']:
            lines.append("[GIT]")
            lines.append(f"Branch: {git['branch']}")
            lines.append(f"Modified: {git['modified']}")
            lines.append(f"Staged: {git['staged']}")
            lines.append(f"Untracked: {git['untracked']}")
            lines.append(f"Last commit: {git['commit_age']} ({git['last_commit']})")
            lines.append("")
        
        # Resources
        lines.append("[RESOURCES]")
        lines.append(f"Disk: {disk['total_gb']:.0f}G total / {disk['free_gb']:.0f}G free ({disk['usage_percent']:.0f}%)")
        lines.append(f"Workspace size: {disk['workspace_gb']:.1f}G")
        lines.append("")
        
        # Alerts
        alerts = []
        if git['available'] and not git['clean']:
            alerts.append("Uncommitted changes present")
        if disk['usage_percent'] > 80:
            alerts.append("Disk usage above 80%")
        if changes['churn'] > 50:
            alerts.append("High file churn detected")
        
        if alerts:
            lines.append("[ALERTS]")
            for alert in alerts:
                lines.append(f"- {alert}")
            lines.append("")
        
        # Actions
        actions = []
        if git['available'] and not git['clean']:
            actions.append("Commit or stash working tree")
        if disk['usage_percent'] > 80:
            actions.append("Review disk usage and clean up old files")
        
        if actions:
            lines.append("[RECOMMENDED ACTIONS]")
            for action in actions:
                lines.append(f"- {action}")
            lines.append("")
        
        lines.append("Roger. Over.")
        return '\n'.join(lines)
    
    def run(self) -> str:
        """Execute full audit cycle"""
        last_run = self.load_previous_state()
        self.scan_filesystem()
        
        changes = self.analyze_changes()
        git = self.get_git_status()
        disk = self.get_disk_usage()
        
        report = self.generate_report(changes, git, disk)
        
        self.save_current_state()
        return report

import time
import sys

if __name__ == "__main__":
    sentry = SentryAudit()
    
    # Check for --once flag for single-run mode (used by !audit command)
    if "--once" in sys.argv:
        try:
            report = sentry.run()
            print(report, flush=True)
        except Exception as e:
            print(f"Error in audit: {e}", flush=True)
            sys.exit(1)
    else:
        # Daemon mode - continuous monitoring
        print("Starting Sentry Audit Daemon...", flush=True)
        while True:
            try:
                report = sentry.run()
                print(report, flush=True)
            except Exception as e:
                print(f"Error in audit cycle: {e}", flush=True)
            
            # Run every 60 seconds
            time.sleep(60)

