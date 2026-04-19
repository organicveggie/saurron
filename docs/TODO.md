# ToDos

## Docker Hub Rate Limiting

https://docs.docker.com/reference/api/hub/latest/#tag/rate-limiting

If you haven't hit the limit, each request to the API will return the following headers in the response.

- `X-RateLimit-Limit` - The limit of requests per minute.
- `X-RateLimit-Remaining` - The remaining amount of calls within the limit period.
- `X-RateLimit-Reset` - The unix timestamp of when the remaining resets.

If you have hit the limit, you will receive a response status of `429` and the `Retry-After` header in the response.

The `Retry-After` header specifies the number of seconds to wait until you can call the API again.
