import email
from email.message import EmailMessage
from email import policy

msg = email.message_from_string("Subject: hi\nFrom: a@b.c\n\nbody\n", policy=policy.default)
subj = msg["Subject"]
body = msg.get_payload()
out = EmailMessage()
out["To"] = "x@y.z"
out.set_content("hello")
text = out.as_string()
