// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
  integrations: [
    starlight({
      title: 'GPUi Shell',
      social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/andre-brandao/gpui-shell' }],
      head: [
        {
          tag: 'script',
          attrs: {
            src: 'https://a.developing.company/api/script.js',
            'data-site-id': 'dd3d22977696',
            defer: true,
          },
        },
      ],
      sidebar: [
        {
          label: 'Getting Started',
          items: [
            { label: 'Installation', slug: 'guides/installation' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Configuration', slug: 'reference/configuration' },
            { label: 'Bar', slug: 'reference/bar' },
            { label: 'Launcher', slug: 'reference/launcher' },
            { label: 'OSD', slug: 'reference/osd' },
            { label: 'Control Center', slug: 'reference/control-center' },
            { label: 'Theme', slug: 'reference/theme' },
          ],
        },
        {
          label: 'Developers',
          items: [
            { label: 'Development Setup', slug: 'developer/setup' },
            { label: 'Architecture', slug: 'developer/architecture' },
            {
              // Temporary: the module APIs are being stabilized for 1.0.
              label: 'Extending (unstable)',
              items: [
                { label: 'Adding a bar module', slug: 'developer/unstable/bar-module' },
                { label: 'Adding a launcher view', slug: 'developer/unstable/launcher-view' },
              ],
            },
            {
              label: 'Contributing',
              link: 'https://github.com/andre-brandao/gpui-shell/blob/main/CONTRIBUTING.md',
              attrs: { target: '_blank' },
            },
            {
              label: 'Roadmap',
              link: 'https://github.com/andre-brandao/gpui-shell/blob/main/ROADMAP.md',
              attrs: { target: '_blank' },
            },
          ],
        },
      ],
    }),
  ],
});
