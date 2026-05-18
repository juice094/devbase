import sqlite3
import os
from datetime import datetime, timezone

db_path = os.path.expandvars(r"%LOCALAPPDATA%\devbase\registry.db")
conn = sqlite3.connect(db_path)
cursor = conn.cursor()

papers = [
    {
        "id": "cpj2025",
        "title": "CPJ: Explainable Agricultural Pest Diagnosis via Caption-Prompt-Judge with LLM-Judged Refinement",
        "authors": "Wentao Zhang, Tao Fang, Lina Lu, Lifei Wang, Weihe Zhong",
        "venue": "arXiv",
        "year": 2025,
        "pdf_path": None,
        "bibtex": r"""@article{cpj2025,
  title={CPJ: Explainable Agricultural Pest Diagnosis via Caption-Prompt-Judge with LLM-Judged Refinement},
  author={Zhang, Wentao and Fang, Tao and Lu, Lina and Wang, Lifei and Zhong, Weihe},
  journal={arXiv preprint arXiv:2512.24947},
  year={2025},
  url={https://github.com/CPJ-Agricultural/CPJ-Agricultural-Diagnosis}
}""",
        "tags": "agriculture,llm-as-judge,benchmark,cpj"
    },
    {
        "id": "agricm3_2025",
        "title": "Agri-CM3: A Chinese Massive Multi-modal, Multi-level Benchmark for Agricultural Understanding and Reasoning",
        "authors": "Haotian Wang, Yi Guan, Fanshu Meng, Chao Zhao, Lian Yan, Yang Yang, Jingchi Jiang",
        "venue": "ACL",
        "year": 2025,
        "pdf_path": None,
        "bibtex": r"""@inproceedings{wang2025agricm3,
  title={Agri-CM3: A Chinese Massive Multi-modal, Multi-level Benchmark for Agricultural Understanding and Reasoning},
  author={Wang, Haotian and Guan, Yi and Meng, Fanshu and Zhao, Chao and Yan, Lian and Yang, Yang and Jiang, Jingchi},
  booktitle={Proceedings of the 63rd Annual Meeting of the Association for Computational Linguistics (Volume 1: Long Papers)},
  pages={11729--11754},
  year={2025},
  url={https://github.com/HIT-Kwoo/Agri-CM3}
}""",
        "tags": "agriculture,multimodal,benchmark,acl2025,agricm3"
    },
    {
        "id": "agmmu2025",
        "title": "AgMMU: A Comprehensive Agricultural Multimodal Understanding and Reasoning Benchmark",
        "authors": "Aruna Gauba, Irene Pi, Yunze Man, Ziqi Pang, Vikram S. Adve, Yu-Xiong Wang",
        "venue": "arXiv",
        "year": 2025,
        "pdf_path": None,
        "bibtex": r"""@article{gauba2025agmmu,
  title={AgMMU: A Comprehensive Agricultural Multimodal Understanding and Reasoning Benchmark},
  author={Gauba, Aruna and Pi, Irene and Man, Yunze and Pang, Ziqi and Adve, Vikram S and Wang, Yu-Xiong},
  journal={arXiv preprint arXiv:2504.10568},
  year={2025},
  url={https://github.com/AgMMU/AgMMU}
}""",
        "tags": "agriculture,multimodal,benchmark,usda-dialogue,agmmu"
    },
    {
        "id": "agridoctor2025",
        "title": "AgriDoctor: A Multimodal Intelligent Assistant for Agriculture",
        "authors": "Zhang Mingqing et al.",
        "venue": "arXiv",
        "year": 2025,
        "pdf_path": None,
        "bibtex": r"""@article{zhang2025agridoctor,
  title={AgriDoctor: A Multimodal Intelligent Assistant for Agriculture},
  author={Zhang, Mingqing and others},
  journal={arXiv preprint arXiv:2509.17044},
  year={2025}
}""",
        "tags": "agriculture,multimodal,agent,agridoctor"
    }
]

now = datetime.now(timezone.utc).isoformat()

for p in papers:
    cursor.execute("""
        INSERT OR REPLACE INTO papers (id, title, authors, venue, year, pdf_path, bibtex, tags, added_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    """, (p["id"], p["title"], p["authors"], p["venue"], p["year"], p["pdf_path"], p["bibtex"], p["tags"], now))
    print(f"Registered paper: {p['id']}")

conn.commit()
conn.close()
print("Done.")
