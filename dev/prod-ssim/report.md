# abgen prod-comparison scoreboard — 2026-06-19

abgen rev `8e6333d`, platform `windows`.

Three independent axes per bundle — byte-identity, **structural** (classify_pair), and **visual SSIM** (ab-render-harness). The structural axis is what stops a high SSIM from hiding an encoding / ordering / PathID / binding error: the `masked` column counts bundles that pass the SSIM floor yet differ structurally from prod.

## Per-entity

| cid | category | prod ver | bundles | byte-id | structural | min SSIM | masked struct |
|---|---|---|---|---|---|---|---|
| `bafkreia22i2cn5y…` | scene | v36 | 6 | 0 | 6 | 0.692414 | 3 |
| `bafkreic24tz5tsa…` | scene | v44 | 57 | 0 | 39 | 0.720473 | 30 |
| `bafkreiaba2qxp4a…` | wearable | v38 | 3 | 0 | 3 | 0.999497 | 3 |
| `bafkreiabanrbjen…` | wearable | v41 | 3 | 0 | 1 | 0.998773 | 1 |
| `bafkreigb24h3cog…` | emote | v44 | 3 | 0 | 2 | 0.999957 | 0 |

## Per-category roll-up

| category | entities | bundles | byte-id | structural | masked | min SSIM | visual-pass | floor | prod versions |
|---|---|---|---|---|---|---|---|---|---|
| scene | 2 | 63 | 0 | 45 | 33 | 0.692414 | 0/2 | 0.99 | v36×1, v44×1 |
| wearable | 2 | 6 | 0 | 4 | 4 | 0.998773 | 2/2 | 0.99 | v38×1, v41×1 |
| emote | 1 | 3 | 0 | 2 | 0 | 0.999957 | 1/1 | 0.99 | v44×1 |

## Per-bundle detail

### `bafkreia22i2cn5y2ytad7n3lrivwletyqtvhtdtwdjtyskruv57cmhgmjm` (scene, prod v36)

| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |
|---|---|---|---|---|---|---|---|
| `bafybeicvgpbbhusafgqqh…` |  | STRUCTURAL | CAT7 | 0.692414 | visual-broken |  | id_w=18 float_w=0 flip_w=0 tex_w=1 struct_w=19 obj_pairs=19 extra(ours/ref)=0/0 sizemis=8 ids_changed=true tex_rmse=0.00 |
| `bafkreiaqbwvy7cuwhev56…` |  | STRUCTURAL | CAT7 | 0.980869 | visual-degraded |  | id_w=5 float_w=0 flip_w=0 tex_w=0 struct_w=1 obj_pairs=3 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0000  |
| `bafkreigkfoyomgruu7qcp…` |  | STRUCTURAL | CAT7 | 0.997747 | visual-ok | **YES** | id_w=21 float_w=1 flip_w=0 tex_w=1 struct_w=27 obj_pairs=25 extra(ours/ref)=0/0 sizemis=11 ids_changed=true tex_rmse=0.0 |
| `bafkreiap5vcskmaf7foof…` |  | STRUCTURAL | CAT7 | 0.999858 | visual-ok | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=1 obj_pairs=3 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0006  |
| `bafkreiam7j7capgggmn5x…` |  | STRUCTURAL | CAT7 | 1.000000 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=0 struct_w=1 obj_pairs=3 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0000  |
| `bafkreignwn3aulyna2mia…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=9 float_w=0 flip_w=0 tex_w=0 struct_w=7 obj_pairs=9 extra(ours/ref)=0/0 sizemis=4 ids_changed=true tex_rmse=0.0000  |

### `bafkreic24tz5tsatlu7ggigtv64rcanybklgsoculj4nvftdbafiuuudye` (scene, prod v44)

| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |
|---|---|---|---|---|---|---|---|
| `bafybeih6quffoz7h6g522…` |  | STRUCTURAL-tex-far | CAT8 | 0.720473 | visual-broken |  | id_w=8 float_w=38 flip_w=0 tex_w=4 struct_w=25 obj_pairs=28 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.32 |
| `bafybeih44ahtk7niix4mh…` |  | value-noise | CAT6 | 0.997447 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=780 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.008 |
| `bafkreibm5p36qbiu24ace…` |  | STRUCTURAL | CAT7 | 0.999469 | visual-ok | **YES** | id_w=2 float_w=0 flip_w=0 tex_w=1 struct_w=6 obj_pairs=10 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0031 |
| `bafkreif3axlgxq5pumbdc…` |  | STRUCTURAL | CAT7 | 0.999770 | visual-ok | **YES** | id_w=6 float_w=0 flip_w=0 tex_w=2 struct_w=18 obj_pairs=24 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreic7foxu5zs2avk4g…` |  | ordering/id-only | CAT5 | 0.999807 | visual-ok |  | id_w=5 float_w=0 flip_w=0 tex_w=53 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0005 |
| `bafkreidfa4bb574t3k6tv…` |  | value-noise | CAT6 | 0.999863 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=39 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreiepag533osdwk2vc…` |  | ordering/id-only | CAT5 | 0.999916 | visual-ok |  | id_w=5 float_w=0 flip_w=0 tex_w=61 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreibxefote3jeusciw…` |  | value-noise | CAT6 | 0.999931 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0004  |
| `bafkreifb2dzlaium4ojtz…` |  | ordering/id-only | CAT5 | 0.999937 | visual-ok |  | id_w=5 float_w=0 flip_w=0 tex_w=54 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreicisttekuq34l352…` |  | value-noise | CAT6 | 0.999964 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=33 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0001 |
| `bafkreiefntstpu3u6qzbp…` |  | ordering/id-only | CAT5 | 0.999964 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=24 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreidj3bf7midchbgra…` |  | STRUCTURAL | CAT7 | 0.999972 | visual-ok | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=5 struct_w=14 obj_pairs=18 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafybeicdgxlymkqpfqiza…` |  | value-noise | CAT6 | 0.999979 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0004  |
| `bafkreigm6aspqcl7nchxw…` |  | ordering/id-only | CAT5 | 0.999986 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0002  |
| `bafkreibceg5yc5kytliht…` |  | STRUCTURAL | CAT7 | 0.999986 | visual-ok | **YES** | id_w=7 float_w=0 flip_w=0 tex_w=2 struct_w=32 obj_pairs=30 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreidagr2rnd664szkn…` |  | STRUCTURAL | CAT7 | 0.999986 | visual-ok | **YES** | id_w=5 float_w=4 flip_w=0 tex_w=2 struct_w=43 obj_pairs=41 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreide7e4f6sqmun25z…` |  | STRUCTURAL | CAT7 | 0.999986 | visual-ok | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=2 struct_w=13 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreig2xm4gzxjnad2ym…` |  | STRUCTURAL | CAT7 | 0.999986 | visual-ok | **YES** | id_w=4 float_w=3 flip_w=0 tex_w=2 struct_w=34 obj_pairs=37 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreigovfdxo4z4daxwo…` |  | value-noise | CAT3 | 0.999986 | visual-ok |  | id_w=5 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0003  |
| `bafybeihibucmuz5ijunaz…` |  | ordering/id-only | CAT5 | 0.999987 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0002  |
| `bafybeigfnvinayc262zn7…` |  | ordering/id-only | CAT5 | 0.999988 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0002  |
| `bafybeihkslk2vjpyqmsn6…` |  | STRUCTURAL | CAT7 | 0.999988 | visual-ok | **YES** | id_w=6 float_w=0 flip_w=0 tex_w=1 struct_w=29 obj_pairs=27 extra(ours/ref)=0/0 sizemis=2 ids_changed=true tex_rmse=0.001 |
| `bafkreiav3zyb6vody64gt…` |  | value-noise | CAT6 | 0.999993 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0002  |
| `bafkreid3ztllijyhra775…` |  | STRUCTURAL | CAT7 | 0.999994 | visual-identical | **YES** | id_w=5 float_w=0 flip_w=6 tex_w=2 struct_w=22 obj_pairs=24 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreidzr6ankbumaiw5c…` |  | ordering/id-only | CAT5 | 0.999994 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0001  |
| `bafkreiessijzsda23xojf…` |  | STRUCTURAL | CAT7 | 0.999994 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=2 struct_w=5 obj_pairs=10 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0001 |
| `bafkreiaqqinihycs5ahud…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=6 float_w=171 flip_w=0 tex_w=3 struct_w=66 obj_pairs=48 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |
| `bafkreiccazb6rdvex2q5d…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=8 float_w=170 flip_w=0 tex_w=3 struct_w=64 obj_pairs=48 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |
| `bafkreicfrzxevc4eq7gtb…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=3 struct_w=23 obj_pairs=27 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreickcqquuytobhkc3…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=10 float_w=169 flip_w=0 tex_w=3 struct_w=59 obj_pairs=48 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0. |
| `bafkreieqdmqw7ohg5wuse…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=6 float_w=170 flip_w=0 tex_w=5 struct_w=61 obj_pairs=48 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |
| `bafkreifbfawml5e3kv4gu…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=5 float_w=1 flip_w=0 tex_w=3 struct_w=23 obj_pairs=27 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreibvpocch7j6n3xem…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=13 float_w=2 flip_w=0 tex_w=4 struct_w=56 obj_pairs=45 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.00 |
| `bafkreicz56wvwao537yh3…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=2 struct_w=6 obj_pairs=10 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0001 |
| `bafkreid5txzpunkjvp2wx…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=2 struct_w=14 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreie4n7gmlpdx6hhw2…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=2 struct_w=6 obj_pairs=10 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0001 |
| `bafkreihv4jqhpjiiccfwa…` |  | STRUCTURAL | CAT7 | 0.999995 | visual-identical | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=2 struct_w=14 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreid5f3qxounkrh7kw…` |  | STRUCTURAL | CAT7 | 0.999997 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=1 struct_w=25 obj_pairs=20 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreifymlqcxokuqhkup…` |  | STRUCTURAL | CAT7 | 0.999998 | visual-identical | **YES** | id_w=3 float_w=0 flip_w=0 tex_w=2 struct_w=5 obj_pairs=10 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0001 |
| `bafkreieihlxu4k77u7gfq…` |  | STRUCTURAL | CAT7 | 0.999998 | visual-identical | **YES** | id_w=4 float_w=0 flip_w=0 tex_w=2 struct_w=19 obj_pairs=20 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreiel45isn4ipmthjs…` |  | STRUCTURAL | CAT7 | 0.999998 | visual-identical | **YES** | id_w=3 float_w=1 flip_w=0 tex_w=2 struct_w=11 obj_pairs=15 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreidqn7wus6pzrvv25…` |  | STRUCTURAL | CAT7 | 0.999998 | visual-identical | **YES** | id_w=4 float_w=2 flip_w=0 tex_w=2 struct_w=15 obj_pairs=20 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreihscin6gkdrjxghp…` |  | STRUCTURAL | CAT7 | 0.999998 | visual-identical | **YES** | id_w=4 float_w=0 flip_w=0 tex_w=2 struct_w=17 obj_pairs=20 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreie23rirhuqc6cbfs…` |  | ordering/id-only | CAT2 | 1.000000 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=0 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000  |
| `bafkreifj7edyuksyatx5r…` |  | STRUCTURAL | CAT7 | 1.000000 | visual-identical | **YES** | id_w=4 float_w=0 flip_w=0 tex_w=3 struct_w=15 obj_pairs=19 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreifxxnsduc4yrrdxd…` |  | ordering/id-only | CAT5 | 1.000000 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=12 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreig4bps3cn7npd23w…` |  | STRUCTURAL | CAT7 | 1.000000 | visual-identical | **YES** | id_w=4 float_w=0 flip_w=0 tex_w=3 struct_w=15 obj_pairs=19 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreigioq22szmj4d6nc…` |  | STRUCTURAL | CAT7 | 1.000000 | visual-identical | **YES** | id_w=4 float_w=1 flip_w=0 tex_w=3 struct_w=14 obj_pairs=19 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreihhobfaiwkvj6cpy…` |  | value-noise | CAT6 | 1.000000 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=0 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000  |
| `bafkreicrlkbki2hnwvhjb…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=3 float_w=0 flip_w=0 tex_w=0 struct_w=20 obj_pairs=23 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreidfir2nfh3jgwi37…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=2 float_w=0 flip_w=0 tex_w=0 struct_w=5 obj_pairs=9 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0000  |
| `bafkreidrmfipfae4adqiu…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=4 float_w=0 flip_w=0 tex_w=0 struct_w=13 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreifazlkohjfrdclre…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=5 float_w=0 flip_w=0 tex_w=0 struct_w=18 obj_pairs=23 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreifhh7dhmilyqvyow…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=2 float_w=0 flip_w=0 tex_w=0 struct_w=21 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreihbmja4yijvoqfuq…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=4 float_w=0 flip_w=0 tex_w=0 struct_w=13 obj_pairs=17 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |
| `bafkreihqhy2jk2l2w3gcm…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=2 float_w=0 flip_w=0 tex_w=0 struct_w=5 obj_pairs=9 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0000  |
| `bafkreihzteekl4trapql6…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=5 float_w=0 flip_w=0 tex_w=0 struct_w=18 obj_pairs=23 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.000 |

### `bafkreiaba2qxp4a5ksk3kzjlszg7ccawhiur5imxmod7jx77nrenodhpie` (wearable, prod v38)

| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |
|---|---|---|---|---|---|---|---|
| `bafybeieg3hekadryfsywl…` |  | STRUCTURAL | CAT7 | 0.999497 | visual-ok | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=0 struct_w=1 obj_pairs=3 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0001  |
| `bafybeiclgwxmjd3j7z4bk…` |  | STRUCTURAL | CAT7 | 0.999996 | visual-identical | **YES** | id_w=17 float_w=8 flip_w=0 tex_w=1 struct_w=190 obj_pairs=135 extra(ours/ref)=0/0 sizemis=5 ids_changed=true tex_rmse=0. |
| `bafkreigztcvh6fclfskzw…` |  | STRUCTURAL | CAT7 | 1.000000 | visual-identical | **YES** | id_w=5 float_w=0 flip_w=0 tex_w=0 struct_w=1 obj_pairs=3 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0000  |

### `bafkreiabanrbjenczq6al77bzv3rc4fptj2s7cwooi7cdt3bgpczgfdfcy` (wearable, prod v41)

| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |
|---|---|---|---|---|---|---|---|
| `bafybeid67rvpghi7vkwfq…` |  | STRUCTURAL | CAT7 | 0.998773 | visual-ok | **YES** | id_w=9 float_w=8 flip_w=0 tex_w=5 struct_w=217 obj_pairs=155 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |
| `bafkreidszwnznrf2t3hrt…` |  | ordering/id-only | CAT5 | 0.999935 | visual-ok |  | id_w=3 float_w=0 flip_w=0 tex_w=127 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.000 |
| `bafkreiefchzjbqi4xyuiy…` |  | value-noise | CAT6 | 1.000000 | visual-identical |  | id_w=3 float_w=0 flip_w=0 tex_w=0 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000  |

### `bafkreigb24h3cog26hwa53gdyehdj44j4vorknl425uf526fg447axnqbm` (emote, prod v44)

| bundle | byte-id | structural bucket | CAT | min SSIM | visual band | masked? | evidence |
|---|---|---|---|---|---|---|---|
| `bafkreihnqso567d26aeja…` |  | value-noise | CAT6 | 0.999957 | visual-ok |  | id_w=5 float_w=0 flip_w=0 tex_w=60 struct_w=0 obj_pairs=3 extra(ours/ref)=0/0 sizemis=0 ids_changed=true tex_rmse=0.0000 |
| `bafkreib3bhymwux23ybpr…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=8 float_w=1 flip_w=0 tex_w=0 struct_w=189 obj_pairs=134 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |
| `bafkreie23c6syinuvkh2c…` |  | STRUCTURAL | CAT7 | n/a | no-texture |  | id_w=7 float_w=4 flip_w=0 tex_w=0 struct_w=188 obj_pairs=134 extra(ours/ref)=0/0 sizemis=1 ids_changed=true tex_rmse=0.0 |

