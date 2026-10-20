# Starter rubric for the wiki audit (tune as needed)

1. /mnt/session/outputs/audit.md and /mnt/session/outputs/findings.json both exist, and findings.json is a valid JSON array with the keys category, path, line, excerpt, problem, confidence, fix.
2. Every file under wiki/ was read; audit.md states the number of files read and it matches the number of files under wiki/.
3. audit.md has a summary with counts per category 1-6 and an overall verdict on readiness for republishing.
4. Every finding names a path and line that exist in the repository, and its excerpt appears at that place.
5. No excerpt is longer than 20 words.
6. Every finding has a concrete fix, not "review this".
7. Each of the six categories is covered; a category without findings says so explicitly.
8. audit.md and findings.json list the same findings.
