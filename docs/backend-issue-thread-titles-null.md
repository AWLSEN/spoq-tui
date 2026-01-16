# Backend Issue: Thread Titles Returning Null

## Date
2026-01-16

## Endpoint
```
GET http://100.80.115.93:8000/v1/threads
```

## Issue Summary
The `title` field is returning `null` for all threads, while `description` is correctly populated with LLM-generated content.

## Expected Behavior
Based on the recent backend update, both `title` and `description` should be populated from thread metadata:
- `title`: Read from `metadata.title`
- `description`: Read from `metadata.description`

## Actual Behavior
- `description`: ✅ Correctly populated with LLM-generated descriptions
- `title`: ❌ Returns `null` for all threads

## Example API Response

```json
{
  "threads": [
    {
      "id": "18020c3b-0528-4b96-a4df-8a0a6199a6c2",
      "type": "normal",
      "title": null,
      "description": "User asks about the AI model identity and requests a long story. The assistant identifies itself as Claude Sonnet 4.5 and explains its design focus on concise, task-oriented assistance for software engineering rather than long-form content generation.",
      "model": null,
      "permission_mode": null,
      "last_activity": "2026-01-16T08:27:26Z",
      "message_count": 6,
      "created_at": "2026-01-16T08:26:02Z"
    },
    {
      "id": "a809bd62-a35e-4fb5-95f0-e5f5e466493c",
      "type": "normal",
      "title": null,
      "description": "User greeting and introduction to a conversation about getting help with the SPOQ Conductor project.",
      "model": null,
      "permission_mode": null,
      "last_activity": "2026-01-16T08:23:40Z",
      "message_count": 2,
      "created_at": "2026-01-16T08:23:33Z"
    },
    {
      "id": "71c071c0-8fc0-487d-8f56-2b3a51d5dd80",
      "type": "normal",
      "title": null,
      "description": "User introduces themselves and asks about the AI model being used, receiving information about Claude Opus 4.5.",
      "model": null,
      "permission_mode": null,
      "last_activity": "2026-01-16T08:16:33Z",
      "message_count": 10,
      "created_at": "2026-01-16T08:10:03Z"
    }
  ],
  "total": 32
}
```

## Questions for Backend Team

1. Is the LLM title generator being triggered? (descriptions are generated, so the LLM pipeline seems to work)

2. Is the title being stored in `metadata.title`? Can you verify in the database that this field is populated?

3. Is there a different code path for title vs description generation that might explain why one works and the other doesn't?

4. For the `thread_updated` WebSocket event - will it include the `title` field once this is fixed?

## Frontend Status
The frontend is ready to receive and display titles. No changes needed on our end once the backend returns the `title` field correctly.
