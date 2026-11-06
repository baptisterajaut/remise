-- Remise runs Pandoc with --sandbox, which reads nothing but the input file. This filter
-- (filters are not sandboxed) lets the document's own images back in: images the reader
-- already holds (inside a .docx or .epub), and relative paths that stay inside the input's
-- folder. Any other image (absolute path, .., file:// or web URL, missing) becomes its alt
-- text, because PDF output fetches images outside the sandbox.
-- CBA: a symlink inside the folder is followed; Pandoc's Lua has no lstat.
local dir = pandoc.path.directory(PANDOC_STATE.input_files[1])

function Image(img)
  local src = img.src
  if pandoc.mediabag.lookup(src) then
    return nil
  end
  if not (src:find("^%a[%w+.-]*:") or src:find("^[/\\]") or src:find("%.%.")) then
    local f = io.open(pandoc.path.join({ dir, src }), "rb")
    if f then
      pandoc.mediabag.insert(src, nil, f:read("a"))
      f:close()
      return nil
    end
  end
  return img.caption
end
