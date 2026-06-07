---
layout: page
title: Container Images
---

<!-- prettier-ignore-start -->
# Container Images
{: .no_toc }

<!-- prettier-ignore-end -->

## Supported Tags

{% assign v  = site.data.release.version %}
{% assign mm = site.data.release.major_minor %}
{% assign m  = site.data.release.major %}
{% assign base = "https://github.com/organicveggie/saurron/blob/main/docker" %}

- [`{{ v }}-trixie`]({{ base }}/trixie/full/Dockerfile), [`{{ mm }}-trixie`]({{ base }}/trixie/full/Dockerfile), [`{{ m }}-trixie`]({{ base }}/trixie/full/Dockerfile), [`{{ v }}`]({{ base }}/trixie/full/Dockerfile), [`{{ mm }}`]({{ base }}/trixie/full/Dockerfile), [`{{ m }}`]({{ base }}/trixie/full/Dockerfile), [`latest`]({{ base }}/trixie/full/Dockerfile)

- [`{{ v }}-slim-trixie`]({{ base }}/trixie/slim/Dockerfile), [`{{ mm }}-slim-trixie`]({{ base }}/trixie/slim/Dockerfile), [`{{ m }}-slim-trixie`]({{ base }}/trixie/slim/Dockerfile), [`slim-{{ v }}`]({{ base }}/trixie/slim/Dockerfile), [`slim-{{ mm }}`]({{ base }}/trixie/slim/Dockerfile), [`slim-{{ m }}`]({{ base }}/trixie/slim/Dockerfile), [`slim`]({{ base }}/trixie/slim/Dockerfile)

- [`{{ v }}-bookworm`]({{ base }}/bookworm/full/Dockerfile), [`{{ mm }}-bookworm`]({{ base }}/bookworm/full/Dockerfile), [`{{ m }}-bookworm`]({{ base }}/bookworm/full/Dockerfile)

- [`{{ v }}-slim-bookworm`]({{ base }}/bookworm/slim/Dockerfile), [`{{ mm }}-slim-bookworm`]({{ base }}/bookworm/slim/Dockerfile), [`{{ m }}-slim-bookworm`]({{ base }}/bookworm/slim/Dockerfile)

- [`{{ v }}-bullseye`]({{ base }}/bullseye/full/Dockerfile), [`{{ mm }}-bullseye`]({{ base }}/bullseye/full/Dockerfile), [`{{ m }}-bullseye`]({{ base }}/bullseye/full/Dockerfile)

- [`{{ v }}-slim-bullseye`]({{ base }}/bullseye/slim/Dockerfile), [`{{ mm }}-slim-bullseye`]({{ base }}/bullseye/slim/Dockerfile), [`{{ m }}-slim-bullseye`]({{ base }}/bullseye/slim/Dockerfile)

## Quick Reference

* Where to file issues:  
  [https://github.com/organicveggie/saurron/issues](https://github.com/organicveggie/saurron/issues)

* Supported architectures: ([more info](https://github.com/docker-library/official-images#architectures-other-than-amd64))  
  `linux/amd64`, `linux/arm64`

* Repository:
  [github.com/organicveggie/saurron](https://github.com/organicveggie/saurron)
