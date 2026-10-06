---
name: worker/python
description: Python scripts and services (ingest jobs, MCP servers, small web servers).
roles: [worker, reviewer]
tags: [python, psycopg, sql, scripts]
paths: ["**/*.py"]
---
# Worker: Python

1. (2026-10-04) psycopg 3: `conn.execute(sql, params).fetchall()`; `Connection.fetchall()` does not exist.
2. (2026-10-04) Never build SQL with f-strings around values or filters; pass parameters.
3. (2026-10-04) A `while ...: ... break` loop that splits text must keep the last piece; check the
   output has all the input (count characters or lines).
4. (2026-10-04) Keep code blocks whole when chunking markdown; never drop lines you cannot parse.
5. (2026-10-04) A 300-line script in one draft comes back half-done ("I will assume..." and `pass`):
   2-4 functions with exact signatures per draft.
6. (2026-10-06) Do what the prompt says word for word: "match os.path.basename(out)" means the
   base name, not the full path; "add to the module docstring" means the one at the top of the file,
   not the function's. Every listed change is part of the answer (a comment line too).
