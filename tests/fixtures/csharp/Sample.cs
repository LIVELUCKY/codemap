using System;

namespace Sample
{
    interface IGreeter
    {
        string Greet(string name);
    }

    class Base {}

    class BookStore : Base, IGreeter
    {
        public string Greet(string name)
        {
            return Load(name);
        }

        const string ApiBase = "/api/v1/books";
    }
}
