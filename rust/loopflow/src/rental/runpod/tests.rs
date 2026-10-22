use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};

use super::{CreatePod, RunPod};
use crate::rental::{Hardware, Template, Usd};

fn at(text: &str) -> time::OffsetDateTime {
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339).unwrap()
}

type Log = Rc<RefCell<Vec<(String, String, Option<Value>)>>>;

/// A RunPod whose replies come from `reply(method, url)` and whose requests are kept.
fn stub(reply: impl Fn(&str, &str) -> std::io::Result<(u16, Value)> + 'static) -> (RunPod, Log) {
    let log: Log = Rc::default();
    let kept = log.clone();
    let api = RunPod::with_send(Box::new(move |method, url, body| {
        let body = body.map(|body| serde_json::from_slice(&body).unwrap());
        kept.borrow_mut()
            .push((method.to_string(), url.to_string(), body));
        reply(method, url).map(|(status, value)| (status, value.to_string().into_bytes()))
    }));
    (api, log)
}

fn pod(name: &str) -> Value {
    json!({
        "id": "pod1",
        "name": name,
        "cost": 0.24,
        "createdAt": "2026-10-21T20:10:00Z",
        "status": "RUNNING",
        "publicIp": "203.0.113.7",
        "portMappings": {"22": 40022},
    })
}

fn template() -> Template {
    Template {
        gpu: None,
        vcpus: 8,
        memory_gb: 16,
        disk_gb: None,
        image: "runpod/base@sha256:ab".into(),
        data_center: Some("US-KS-2".into()),
    }
}

#[test]
fn a_cpu_template_is_quoted_from_the_catalog_in_its_data_center() {
    let (api, log) = stub(|_, _| {
        Ok((
            200,
            json!({"cpus": [
                {"id": "cpu3c", "vcpu": {"min": 2, "max": 32}, "ramGbPerVcpu": 2,
                 "price": {"securePerVcpu": 0.03},
                 "dataCenters": [{"id": "US-KS-2", "availability": "HIGH"}]},
                {"id": "cpu5c", "vcpu": {"min": 2, "max": 32}, "ramGbPerVcpu": 2,
                 "price": {"securePerVcpu": 0.02},
                 "dataCenters": [{"id": "EU-SE-1", "availability": "HIGH"}]}
            ]}),
        ))
    });
    let quote = api.quote(&template()).unwrap();
    assert_eq!(quote.hardware.to_string(), "cpu3c×8");
    assert_eq!(quote.usd_per_hour, Usd::parse("0.24").unwrap());
    assert!(log.borrow()[0].1.contains("vcpuCount=8"));
}

#[test]
fn create_asks_for_exactly_the_quoted_cpu_and_reads_the_ssh_address() {
    let (api, log) = stub(|_, _| Ok((200, pod("lf-rent_1"))));
    let created = api
        .create(&CreatePod {
            name: "lf-rent_1".into(),
            hardware: Hardware::Cpu {
                flavor: "cpu3c".into(),
                vcpus: 8,
            },
            image: template().image,
            disk_gb: 40,
            data_center: None,
            env: vec![("PUBLIC_KEY".into(), "ssh-ed25519 AAAA".into())],
            entrypoint: vec!["bash".into()],
        })
        .unwrap();
    assert_eq!(created.ssh, Some(("203.0.113.7".into(), 40022)));
    let sent = log.borrow()[0].2.clone().unwrap();
    assert_eq!(sent["cpu"], json!({"id": "cpu3c", "vcpuCount": 8}));
    assert!(sent.get("gpu").is_none() && sent.get("mounts").is_none());
    assert_eq!(sent["ports"], json!(["22/tcp"]));
}

#[test]
fn create_refuses_an_unpinned_image_or_the_account_key_without_asking() {
    let (api, log) = stub(|_, _| Ok((200, pod("x"))));
    let mut request = CreatePod {
        name: "x".into(),
        hardware: Hardware::Cpu {
            flavor: "cpu3c".into(),
            vcpus: 2,
        },
        image: "runpod/base:latest".into(),
        disk_gb: 20,
        data_center: None,
        env: Vec::new(),
        entrypoint: Vec::new(),
    };
    assert!(api.create(&request).is_err());
    request.image = template().image;
    request.env = vec![("RUNPOD_API_KEY".into(), "k".into())];
    assert!(api.create(&request).is_err());
    assert!(log.borrow().is_empty());
}

#[test]
fn a_create_whose_reply_names_another_pod_is_not_trusted() {
    let (api, _) = stub(|_, _| Ok((200, pod("someone-else"))));
    let request = CreatePod {
        name: "lf-rent_1".into(),
        hardware: Hardware::Cpu {
            flavor: "cpu3c".into(),
            vcpus: 2,
        },
        image: template().image,
        disk_gb: 20,
        data_center: None,
        env: Vec::new(),
        entrypoint: Vec::new(),
    };
    assert!(api.create(&request).is_err());
}

#[test]
fn an_uncertain_create_is_found_by_name_across_inventory_pages() {
    let (api, _) = stub(|_, url| {
        let page = if url.contains("cursor=next") {
            json!({"pods": [pod("lf-rent_1")], "pagination": {"hasNextPage": false}})
        } else {
            json!({"pods": [pod("other")], "pagination": {"hasNextPage": true, "nextCursor": "next"}})
        };
        Ok((200, page))
    });
    assert_eq!(api.named("lf-rent_1").unwrap().unwrap().id, "pod1");
    assert_eq!(api.named("lf-rent_2").unwrap(), None);
}

#[test]
fn no_reply_is_never_read_as_absence() {
    let (api, _) = stub(|_, _| Err(std::io::Error::other("timeout")));
    assert!(api.named("lf-rent_1").is_err());
    assert!(api.pod("pod1").is_err());
}

#[test]
fn delete_rechecks_identity_and_treats_absent_as_given_back() {
    let (api, log) = stub(|_, _| Ok((200, pod("lf-rent_1"))));
    assert!(api.delete("pod1", "lf-rent_2").is_err());
    assert!(log.borrow().iter().all(|(method, _, _)| method == "GET"));
    api.delete("pod1", "lf-rent_1").unwrap();
    assert_eq!(log.borrow().last().unwrap().0, "DELETE");

    let (gone, log) = stub(|_, _| Ok((404, Value::Null)));
    gone.delete("pod1", "lf-rent_1").unwrap();
    assert_eq!(log.borrow().len(), 1);
}

#[test]
fn billing_counts_only_contiguous_whole_hours() {
    let (api, _) = stub(|method, url| {
        if method == "GET" && url.contains("billing/pods") {
            let hour = |start: &str, end: &str, usd: f64| json!({"podId": "pod1", "startTime": start, "endTime": end, "totalAmount": usd});
            Ok((
                200,
                json!({"records": [
                    hour("2026-10-21T20:00:00Z", "2026-10-21T21:00:00Z", 0.2),
                    hour("2026-10-21T21:00:00Z", "2026-10-21T22:00:00Z", 0.24),
                    hour("2026-10-21T23:00:00Z", "2026-10-22T00:00:00Z", 0.24),
                ]}),
            ))
        } else {
            Ok((200, pod("lf-rent_1")))
        }
    });
    let pod = api.pod("pod1").unwrap().unwrap();
    let billed = api.billed(&pod, at("2026-10-22T00:30:00Z")).unwrap();
    assert_eq!(billed, Usd::parse("0.44").unwrap());
    assert_eq!(
        api.billed(&pod, at("2026-10-21T20:40:00Z")).unwrap(),
        Usd::ZERO
    );
}

#[test]
fn the_pod_boots_its_guardian_with_the_deadline_and_cap_but_no_account_key() {
    let (env, entrypoint) = super::boot(
        at("2026-10-22T00:10:00Z"),
        Usd::parse("0.96").unwrap(),
        "ssh-ed25519 AAAA jack\n",
    );
    let value = |key: &str| env.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(value("LF_DEADLINE"), Some("2026-10-22T00:10:00Z"));
    assert_eq!(value("LF_MAX_USD"), Some("0.96"));
    assert_eq!(value("PUBLIC_KEY"), Some("ssh-ed25519 AAAA jack"));
    assert_eq!(value("RUNPOD_API_KEY"), None);
    let boot = &entrypoint[2];
    let guardian = boot.find("python3 /opt/lf/guardian.py").unwrap();
    assert!(guardian < boot.find("sshd").unwrap());
    assert!(guardian < boot.find("apt-get").unwrap());
}
