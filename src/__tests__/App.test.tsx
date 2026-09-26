import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "@/App";

describe("App", () => {
  it("renders without crashing", () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText("SourceForge")).toBeInTheDocument();
  });

  it("renders Sidebar with 5 collapsible groups", () => {
    render(
      <MemoryRouter initialEntries={["/runner"]}>
        <App />
      </MemoryRouter>
    );
    const groups = screen.getAllByRole("button", { name: /(WORK|REVIEW|CONFIGURE|AUTOMATE|SYSTEM)/ });
    expect(groups).toHaveLength(5);
  });

  it("redirects / to /runner", () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText("Runner")).toBeInTheDocument();
  });

  it("renders placeholders for unknown route", () => {
    render(
      <MemoryRouter initialEntries={["/unknown"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText(/under construction/)).toBeInTheDocument();
  });

  it("renders theme toggle in sidebar", () => {
    render(
      <MemoryRouter initialEntries={["/"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText("Dark")).toBeInTheDocument();
    expect(screen.getByText("Light")).toBeInTheDocument();
  });

  it.each(["/route", "/assets", "/outcomes", "/playbooks", "/knowledge", "/scheduler", "/costs", "/settings"])(
    "%s renders without error",
    (route) => {
      render(
        <MemoryRouter initialEntries={[route]}>
          <App />
        </MemoryRouter>
      );
      expect(document.querySelector("main")).not.toBeEmptyDOMElement();
    }
  );

  it("redirects /handoffs → /orchestrate/handoffs", () => {
    render(
      <MemoryRouter initialEntries={["/handoffs"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText(/Build Handoff Envelope/i)).toBeInTheDocument();
  });

  it("redirects /messages → /orchestrate/messages", () => {
    render(
      <MemoryRouter initialEntries={["/messages"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText(/Parse ACB Signal/i)).toBeInTheDocument();
  });

  it("redirects /connectors → /integrations", () => {
    render(
      <MemoryRouter initialEntries={["/connectors"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByRole("heading", { name: /Integrations/i })).toBeInTheDocument();
  });
});
