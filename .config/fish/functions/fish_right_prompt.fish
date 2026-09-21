function fish_right_prompt
  # Display cached git info (updated asynchronously)
  if set -q _async_git_info
    echo -n -s $_async_git_info
  end
end

function _update_git_info --on-event fish_prompt
  # Kill any existing background job
  if set -q _async_git_job_pid
    kill $_async_git_job_pid 2>/dev/null
  end

  # Run git prompt in background and update when ready
  fish -c 'fish_git_prompt " %s"' > /tmp/fish_git_info_$fish_pid 2>/dev/null &
  set -g _async_git_job_pid $last_pid

  # Monitor the background job and update when ready
  function _async_git_update_watcher --on-process-exit $_async_git_job_pid
    if test -f /tmp/fish_git_info_$fish_pid
      set -g _async_git_info (cat /tmp/fish_git_info_$fish_pid)
      rm -f /tmp/fish_git_info_$fish_pid
    else
      set -e _async_git_info
    end
    set -e _async_git_job_pid
    commandline -f repaint 2>/dev/null
    functions -e _async_git_update_watcher
  end
end
