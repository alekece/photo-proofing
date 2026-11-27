# Find photos and activity by name

Approved on 2026-11-24. Tracked by [issue #421](https://github.com/alekece/photo-proofing/issues/421).

Search names rather than placing an expanding list of galleries or visitors in a menu.
Keep menus for fixed choices.

| Screen | Search matches | Other controls |
| --- | --- | --- |
| Photos | Gallery title | Photo type and sort |
| Workspace Activity | Gallery title or visitor name | Date, event type and sort |
| Gallery Activity | Visitor name in the current gallery | Date, event type and sort |

Reuse the shared toolbar search field. Match partial names regardless of case or accents.
The anonymous visitor label is searchable too. Clearing the search leaves the selected filters
and sort intact. Searching filenames, comment text or event labels is out of scope.

Photos already has gallery-name search over its loaded library. Activity should apply search on
the server before pagination, restricted to the history the current view can already access.
Filtering only loaded Activity rows would be simpler but could report no matches while an older
matching event exists. Keep the existing sort and pagination behavior for matching rows.

Activity uses PostgreSQL's `unaccent` extension. Verify it is available to the migration role
and visible to the app before deployment. An image-only rollback to a version that predates
the migration is not supported.

Validate the journeys first in the desktop preview with synthetic data. No mobile redesign.
