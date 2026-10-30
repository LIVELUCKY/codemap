import { fetchBook } from "./api";

test("fetches an book", async () => {
  await fetchBook("1");
});
