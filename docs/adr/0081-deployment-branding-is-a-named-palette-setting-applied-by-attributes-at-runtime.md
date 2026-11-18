# Deployment branding is a named-palette setting in the Country Configuration package, applied by attributes at runtime

Decided in [How is deployment branding (accent, gray, radius, font, logo) modelled and applied with the light, dark and high-contrast themes?](https://github.com/carapai/people-centered-healthcare/issues/646), on the map [Wayfinder: a professional, themeable design system (Park UI)](https://github.com/carapai/people-centered-healthcare/issues/643). It builds on [ADR 0080](0080-the-design-system-copies-park-ui-recipes-as-source-onto-radix-colors.md) (Radix Colors, Park UI's `colorPalette`) and on Park UI's defaults ([Publish the design system backlog](https://github.com/carapai/people-centered-healthcare/issues/648)).

One build serves every country, central and the edges, so a deployment's look cannot be compiled in. It must also never break WCAG 2.2 AA, the high-contrast theme or offline use.

**The decision:**
- **Branding has five fields:**
  - an **accent** palette, named from Radix's scales;
  - a **gray** palette, one of Radix's six grays (gray, mauve, slate, sage, olive, sand);
  - a **radius**, one of Park UI's radius options;
  - a **product name**, shown in the header, the page title and on sign-in;
  - **logos**: a required logo (light), an optional dark logo (dark and high contrast) and a required square mark (sidebar, favicon, desktop icon).

  There are no free hex colours and no per-component overrides.
- **Contrast is proven ahead of time.**
  - A design-system test runs every accent × gray in light and dark through the 4.5:1 text and 3:1 control-border checks.
  - The test fixes, for each scale, the step used as the solid fill and whether it takes white or dark text. In light mode the solid fill is step 11, so white text passes.
  - Scales that cannot pass in a mode are left off the **allowed list**, and publication rejects a package that names one.
- **Where it lives:**
  - `configuration/branding.json` in the **Country Configuration** package, validated by a schema the software release owns. The logos travel in the same package.
  - One branding per deployment. There is no override point for districts or facilities.
  - Changing it means publishing and activating a new package version, with one approver.
- **Applied at runtime without a rebuild:**
  - The bundle carries every allowed scale as CSS variables, keyed by `[data-accent=…]`, `[data-gray=…]` and `[data-radius=…]` (about 30 KB).
  - The node serves the active branding at `/branding.json` and the logos under `/branding/`.
  - The client fetches them alongside `runtime.json`, before its first render, and sets the attributes on `<html>`. Nothing is generated at runtime and there are no inline styles, so the Content-Security-Policy is unchanged and there is no flash of the default look.
- **Themes and density:**
  - Accent and gray apply in light and dark.
  - **High contrast ignores accent and gray** and stays black, white and yellow; it keeps radius and logo.
  - Density is independent of branding.
- **Logos are inert SVG.** They are drawn with `<img>`. Publication rejects a logo with `<script>` or `<foreignObject>`, an external reference, or a size over 64 KB. Without a dark logo, dark and high contrast show the product name as text.
- **Default:** with no branding, a deployment gets Park UI's default accent, gray and radius and "Health Platform".
- **Offline:**
  - Edges receive branding with the package through sync.
  - The service worker caches `/branding.json` and the logos with the app, and desktops get them from their edge.
  - Newly activated branding shows on the next app load, never mid-session.
- **Fonts and preview are deferred.** Park UI's default font stack is used and branding sets no font; a Studio preview of a draft branding waits until deployments need it. Publication's contrast and logo checks still apply.

## Considered options

- **Any hex colour, with scales generated at runtime:** contrast could no longer be proven ahead of time. It would also need runtime CSS, which conflicts with the Content-Security-Policy and Panda's static extraction.
- **A build per deployment:** this breaks the one-bundle rule that central and the edges share, and every rebrand would need a release.
- **Branding as a facility override point:** facilities would look different across referrals and shared screens, and the header already names the facility.
- **Letting branding recolour high contrast:** high contrast exists for users who need it, so a brand colour must not weaken it.
