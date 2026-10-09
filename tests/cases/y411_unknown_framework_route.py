"""A view registered through an unknown framework's decorator and never called here: it runs
with an open request.

Assertions inside functions describe every run of the line: each call, or, for a function
nobody calls, a run with open parameters. Expectations were written from the program text
before running the engine (precise answers; soundness floors where the precise answer may be
out of reach)."""
import framework

app = framework.App()


@app.route("/x")
def view(request):
    data = {"path": request.path}            #@ input path
    if request.args.get("q"):
        data["q"] = request.args["q"]        #@ ok q
    _ = 0                                    #@row data: present={path,q} definite={path} closed
    return data
