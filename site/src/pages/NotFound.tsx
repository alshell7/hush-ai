import { BookIcon } from "../components/Icons";
import { Mochi } from "../components/Mochi";
import { DOCS_HREF, homeHref } from "../lib/route";

export function NotFound() {
  return (
    <div className="wrap grid min-h-[60vh] place-items-center py-20 text-center">
      <div>
        <div className="mx-auto w-fit">
          <Mochi size={96} mood="oops" breathe={false} />
        </div>
        <h1 className="heading mt-6 text-[40px] sm:text-[56px]">This page is too quiet</h1>
        <p className="lede mx-auto mt-4 max-w-[460px] text-[17px]">We could not find it. It may have moved.</p>
        <div className="mt-8 flex flex-col items-center justify-center gap-3 sm:flex-row">
          <a href={homeHref()} className="btn btn-primary btn-lg">
            Back to Hush AI
          </a>
          <a href={DOCS_HREF} className="btn btn-soft btn-lg">
            <BookIcon size={18} /> Docs
          </a>
        </div>
      </div>
    </div>
  );
}
