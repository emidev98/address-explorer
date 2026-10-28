# Apache ECharts (vendored)

- Version: **6.1.0**
- Source: <https://www.npmjs.com/package/echarts/v/6.1.0>
- Bundle: `dist/echarts.min.js`
- Licenses: `LICENSE` (Apache-2.0), `LICENSE-d3` (BSD-3-Clause, from `licenses/` in the tarball)

The bundle is committed so charts load without a CDN or a Node.js build step.
To update it, extract the same files from the new npm tarball, replace them
here, and update the version above.

```sh
curl -sL https://registry.npmjs.org/echarts/-/echarts-6.1.0.tgz -o echarts.tgz
tar -xzf echarts.tgz package/dist/echarts.min.js package/LICENSE package/licenses/LICENSE-d3
```
