# Template Download 📥

Want to pull shared templates from a repo or gist? A simple pattern:

```bash
# Example: fetch a template into your templates dir
curl -L https://awfulsec.com/bigfiles/templates/news_parser.yaml \
  -o "$($AJ_CONFIG_PATH)/templates/news_parser.yaml"
```
You can browse some example templates here: https://awfulsec.com/bigfiles/templates

> Tip: Consider versioning your personal templates in a dotfiles repo. 🔖