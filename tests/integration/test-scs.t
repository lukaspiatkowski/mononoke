  >   export COMMIT_$1="$(hg --debug id -i)" # save hash to variable
  $ hg bookmark -i BOOKMARK_B

A commit with a file change and binary file

  $ echo -e "b\nc\nd\ne\nf" > b
  $ echo -e "\0 10" > binary
  $ commit C
  adding binary

Commit with globalrev:
  $ touch c
  $ hg add
  adding c
  $ hg commit -Am "commit with globalrev" --extra global_rev=9999999999
  $ hg bookmark -i BOOKMARK_C

A commit with file move and copy

  $ hg update -q $COMMIT_B
  $ hg move a moved_a
  $ echo x >> moved_a
  $ hg cp b copied_b
  $ commit D
  $ blobimport repo-hg/.hg repo --has-globalrev
diff
  $ scsc diff --repo repo -B BOOKMARK_B -i "$COMMIT_C"
  diff --git a/b b/b
  --- a/b
  +++ b/b
  @@ -1,5 +1,5 @@
  -a
   b
  +c
   d
   e
   f
  diff --git a/binary b/binary
  new file mode 100644
  Binary file binary has changed

  $ scsc diff --repo repo -i "$COMMIT_B" -i "$COMMIT_D"
  diff --git a/b b/copied_b
  copy from b
  copy to copied_b
  diff --git a/a b/moved_a
  rename from a
  rename to moved_a
  --- a/a
  +++ b/moved_a
  @@ -3,3 +3,4 @@
   c
   d
   e
  +x

lookup using bookmarks

lookup, commit without globalrev
  $ scsc lookup --repo repo  -B BOOKMARK_B -S bonsai,hg,globalrev
  bonsai=c63b71178d240f05632379cf7345e139fe5d4eb1deca50b3e23c26115493bbbb
  hg=323afe77a1b1e632e54e8d5a683ba2cc8511f299

lookup, commit with globalrev
  $ scsc lookup --repo repo -B BOOKMARK_C -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using bonsai to identify commit
  $ scsc lookup --repo repo --bonsai-id 006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using globalrev to identify commit
  $ scsc lookup --repo repo --globalrev 9999999999 -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using hg to identify commit
  $ scsc lookup --repo repo --hg-commit-id ee87eb8cfeb218e7352a94689b241ea973b80402 -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using bonsai needed resolving to identify commit
  $ scsc lookup --repo repo -i 006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using globalrev needed resolving to identify commit
  $ scsc lookup --repo repo -i 9999999999 -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

lookup using hg needed resolving to identify commit
  $ scsc lookup --repo repo -i ee87eb8cfeb218e7352a94689b241ea973b80402 -S bonsai,hg,globalrev
  bonsai=006c988c4a9f60080a6bc2a2fff47565fafea2ca5b16c4d994aecdef0c89973b
  globalrev=9999999999
  hg=ee87eb8cfeb218e7352a94689b241ea973b80402

cat a file
  $ scsc cat --repo repo -B BOOKMARK_B -p a
  a
  b
  c
  d
  e