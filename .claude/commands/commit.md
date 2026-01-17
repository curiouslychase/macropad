# Smart Commit

Analyze changes, group semantically, and create separate PRs for each logical change set.

## Steps

1. **Gather all changes**:
```bash
git status
```
```bash
git diff
```
```bash
git diff --cached
```

2. **Analyze and group changes**:
   - Review all modified, staged, and untracked files
   - Semantically group files into logical change sets based on their purpose (e.g., "display updates", "keyboard mapping", "build config")
   - Generate a descriptive branch name for each group (e.g., `feat/colemak-keyboard-mapping`, `fix/display-layout`)
   - Generate a concise commit message for each group

3. **Save current state**:
```bash
git branch --show-current
```

4. **For each change group**, execute in sequence:
   - Create branch from main: `git checkout -b <branch-name> main`
   - Stage only files in this group: `git add <file1> <file2> ...`
   - Commit with generated message
   - Push to remote: `git push -u origin <branch-name>`
   - Create PR: `gh pr create --title "<title>" --body "<body>"`
   - Capture PR URL
   - Return to main: `git checkout main`

5. **Cleanup**:
   - Return to original branch if different from main
   - Working tree should be clean (all changes now in PRs)

6. **Output**:
   - List all created PRs with their URLs
   - Summarize what was included in each PR
