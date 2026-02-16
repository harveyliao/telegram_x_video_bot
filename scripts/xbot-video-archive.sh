#!/usr/bin/env bash
set -euo pipefail

# Archive Telegram bot downloads:
#   <SRC_DIR>/<YYYY-MM-DD_HH-MM-SS>_<chat_id>/video.mp4
# ->
#   <DST_DIR>/video_YYYY-MM-DD_HH-MM-SS.mp4
# If target exists, append _1, _2, ... before .mp4.

SRC_DIR="${SRC_DIR:-/var/lib/xbot/videos}"
DST_DIR="${DST_DIR:-/data/xbot/videos}"

log() {
  printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"
}

cleanup_task_dir_if_stale() {
  local task_dir="$1"
  local cookie_file="$task_dir/cookie.txt"
  local unexpected

  # Only clean directories that contain no files other than cookie.txt.
  unexpected="$(find "$task_dir" -mindepth 1 -maxdepth 1 ! -name 'cookie.txt' -print -quit)"
  if [[ -n "$unexpected" ]]; then
    return
  fi

  if [[ -f "$cookie_file" ]] && ! rm -f -- "$cookie_file"; then
    log "cleanup: failed to remove $cookie_file"
    return
  fi

  if rmdir -- "$task_dir" 2>/dev/null; then
    log "cleanup: removed stale task directory $task_dir"
  fi
}

if [[ ! -d "$SRC_DIR" ]]; then
  log "source directory does not exist: $SRC_DIR"
  exit 1
fi

mkdir -p -- "$DST_DIR"

if [[ ! -w "$DST_DIR" ]]; then
  log "destination directory is not writable: $DST_DIR"
  exit 1
fi

# Match task dirs like: 2024-03-04_21-12-50_12345 or 2024-03-04_21-12-50_-10012345
TASK_RE='^([0-9]{4}-[0-9]{2}-[0-9]{2}_[0-9]{2}-[0-9]{2}-[0-9]{2})_(-?[0-9]+)$'

moved=0
skipped=0
errors=0

while IFS= read -r -d '' task_dir; do
  dir_name="$(basename -- "$task_dir")"

  if [[ ! "$dir_name" =~ $TASK_RE ]]; then
    ((skipped+=1))
    log "skip: directory name does not match task pattern: $task_dir"
    continue
  fi

  ts="${BASH_REMATCH[1]}"
  src_file="$task_dir/video.mp4"

  if [[ ! -f "$src_file" ]]; then
    ((skipped+=1))
    log "skip: missing video.mp4 in $task_dir"
    cleanup_task_dir_if_stale "$task_dir"
    continue
  fi

  base="video_${ts}"
  target="$DST_DIR/${base}.mp4"

  if [[ -e "$target" ]]; then
    n=1
    while [[ -e "$DST_DIR/${base}_${n}.mp4" ]]; do
      ((n+=1))
    done
    target="$DST_DIR/${base}_${n}.mp4"
  fi

  if mv -- "$src_file" "$target"; then
    ((moved+=1))
    log "moved: $src_file -> $target"
    cleanup_task_dir_if_stale "$task_dir"
  else
    ((errors+=1))
    log "error: failed to move $src_file"
  fi

done < <(find "$SRC_DIR" -mindepth 1 -maxdepth 1 -type d -print0)

log "done: moved=$moved skipped=$skipped errors=$errors"
if (( errors > 0 )); then
  exit 1
fi
