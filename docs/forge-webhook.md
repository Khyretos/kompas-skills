# Pull requests close their tasks (Forgejo or Gitea webhook)

When a pull request names a task, Kompanion moves that task by itself: opened or reopened -> in review, merged -> done, closed without merging -> back to queued (only if it was in review). The page updates without a reload and the usual notifications go out.

## Name the task in the pull request

- Put the task key in square brackets in the PR title: `[TEN-03] Webhook closes tasks`. The key is the end of the task's id (`kompanion-TEN-03` -> `TEN-03`) or the start of its title (`M6-05 Workflow registry` -> `M6-05`).
- Or start the branch with it: `m6-05-workflows`.

## Set it up (once)

1. Make a long random secret: `openssl rand -hex 32`.
2. In Kompanion's `.env` (next to docker-compose.yml) add `FORGE_WEBHOOK_SECRET=<the secret>` and `FORGE_TASK_USER=<your Kompanion user name>` (whose tasks the webhook may move), then restart Kompanion. Without both, the webhook answers 404.
3. In Forgejo: the repository's Settings > Webhooks > Add webhook > Forgejo. Target URL `https://<your Kompanion>/api/forge/webhook`, method POST, content type `application/json`, Secret: the same secret, Trigger on: Custom events > Pull request. Save, then "Test delivery" (a push test is answered with 204 and changes nothing).

## Safety

- Every request must carry a valid HMAC-SHA256 signature of its body (`X-Forgejo-Signature`); anything else gets 401 before the body is read.
- Only FORGE_TASK_USER's tasks can change, and only between the states above.
- Bodies are limited to 1 MB.
