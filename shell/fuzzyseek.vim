" FuzzySeek integration for Vim/Neovim
" Usage: :FuzzySeek [source_command]
"   :FuzzySeekFiles    - find files
"   :FuzzySeekBuffers  - switch buffers
"   :FuzzySeekHistory  - command history
"   :FuzzySeekGrep     - grep in project

if exists('g:loaded_fuzzyseek')
  finish
endif
let g:loaded_fuzzyseek = 1

" Configuration
let g:fuzzyseek_command = get(g:, 'fuzzyseek_command', 'fuzzyseek')
let g:fuzzyseek_options = get(g:, 'fuzzyseek_options', '')
let g:fuzzyseek_file_command = get(g:, 'fuzzyseek_file_command',
      \ "find . -path '*/\\.*' -prune -o -type f -print | sed 's|^\\./||'")

" Core function: run fuzzyseek with a source command and handle result
function! s:fuzzyseek(action, source_cmd, ...) abort
  let l:opts = a:0 > 0 ? a:1 : ''
  let l:tempfile = tempname()

  " Build the command pipeline
  let l:cmd = a:source_cmd . ' | '
  let l:cmd .= g:fuzzyseek_command . ' ' . g:fuzzyseek_options . ' ' . l:opts
  let l:cmd .= ' > ' . shellescape(l:tempfile)

  " Execute in terminal or system()
  if has('nvim')
    " Neovim: use termopen for TTY support
    let l:buf = nvim_create_buf(v:false, v:true)
    call nvim_open_win(l:buf, v:true, {
          \ 'relative': 'editor',
          \ 'width': &columns - 4,
          \ 'height': &lines - 4,
          \ 'col': 2,
          \ 'row': 2,
          \ 'style': 'minimal',
          \ 'border': 'rounded'
          \ })
    call termopen(l:cmd . ' 2>/dev/tty', {
          \ 'on_exit': {job_id, code, event -> s:on_exit(a:action, l:tempfile, code)}
          \ })
    startinsert
  else
    " Vim: use system with shell
    silent execute '!' . l:cmd . ' </dev/tty 2>/dev/tty'
    redraw!
    call s:on_exit(a:action, l:tempfile, v:shell_error)
  endif
endfunction

function! s:on_exit(action, tempfile, exit_code) abort
  if a:exit_code != 0
    if filereadable(a:tempfile)
      call delete(a:tempfile)
    endif
    return
  endif

  if !filereadable(a:tempfile)
    return
  endif

  let l:results = readfile(a:tempfile)
  call delete(a:tempfile)

  if empty(l:results)
    return
  endif

  " Handle result based on action
  for l:item in l:results
    if empty(l:item)
      continue
    endif
    execute a:action . ' ' . fnameescape(l:item)
  endfor
endfunction

" --- Commands ---
command! -nargs=* -complete=file FuzzySeek
      \ call s:fuzzyseek('edit', g:fuzzyseek_file_command, <q-args>)

command! -nargs=0 FuzzySeekFiles
      \ call s:fuzzyseek('edit', g:fuzzyseek_file_command)

command! -nargs=0 FuzzySeekSplit
      \ call s:fuzzyseek('split', g:fuzzyseek_file_command)

command! -nargs=0 FuzzySeekVsplit
      \ call s:fuzzyseek('vsplit', g:fuzzyseek_file_command)

command! -nargs=0 FuzzySeekTab
      \ call s:fuzzyseek('tabedit', g:fuzzyseek_file_command)

command! -nargs=0 FuzzySeekBuffers
      \ call s:fuzzyseek('buffer', s:buffer_source())

command! -nargs=0 FuzzySeekHistory
      \ call s:fuzzyseek('edit', s:history_source())

command! -nargs=+ FuzzySeekGrep
      \ call s:grep(<q-args>)

" --- Buffer source ---
function! s:buffer_source() abort
  let l:bufs = filter(range(1, bufnr('$')), 'buflisted(v:val) && bufname(v:val) != ""')
  let l:names = map(l:bufs, 'bufname(v:val)')
  let l:tempfile = tempname()
  call writefile(l:names, l:tempfile)
  return 'cat ' . shellescape(l:tempfile)
endfunction

" --- History source ---
function! s:history_source() abort
  let l:hist = []
  for l:i in range(1, histnr(':'))
    call add(l:hist, histget(':', l:i))
  endfor
  call reverse(l:hist)
  call filter(l:hist, 'v:val != ""')
  let l:tempfile = tempname()
  call writefile(l:hist, l:tempfile)
  return 'cat ' . shellescape(l:tempfile)
endfunction

" --- Grep integration ---
function! s:grep(pattern) abort
  let l:cmd = 'grep -rn --include="*" ' . shellescape(a:pattern) . ' .'
  let l:tempfile = tempname()
  let l:output = systemlist(l:cmd)
  if empty(l:output)
    echo 'FuzzySeek: no matches found'
    return
  endif
  call writefile(l:output, l:tempfile)
  call s:fuzzyseek('edit', 'cat ' . shellescape(l:tempfile), '--preview "echo {}"')
endfunction

" --- Default mappings ---
if !exists('g:fuzzyseek_no_mappings') || !g:fuzzyseek_no_mappings
  nnoremap <silent> <leader>ff :FuzzySeekFiles<CR>
  nnoremap <silent> <leader>fb :FuzzySeekBuffers<CR>
  nnoremap <silent> <leader>fh :FuzzySeekHistory<CR>
  nnoremap <silent> <leader>fs :FuzzySeekSplit<CR>
  nnoremap <silent> <leader>fv :FuzzySeekVsplit<CR>
  nnoremap <silent> <leader>ft :FuzzySeekTab<CR>
endif
