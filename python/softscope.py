""" Soft scope prototype demo.

Generate a 10 kHz samples sine wave and plot it live.
"""

import math
import time
import queue
import random
import threading
from PyQt5.QtWidgets import QApplication, QWidget, QVBoxLayout
from PyQt5.QtCore import QTimer
from lognplot import ZoomSerie
from lognplot.qt.widgets.chartwidget import ChartWidget


def main():
    app = QApplication([])
    w = DemoGraphWidget()
    t1 = threading.Thread(target=gen_data, args=(w._rx_queue,), daemon=True)
    t1.start()
    w.show()
    app.exec()


def gen_data(tx_queue):
    print("enter thread")
    ts = 0.0001
    t = 0
    dt = 0.2
    F = 400
    F2 = 3
    chunk_size = int(dt / ts)
    while True:

        samples = []
        samples2 = []
        for _ in range(chunk_size):
            v = 4 * math.sin(t * math.pi * 2.0 * F) - 2
            samples.append((t, v))
            v2 = 6 * math.sin(t * math.pi * 2.0 * F2) + 5 + 0.1 * random.random()
            samples2.append((t, v2))

            # Increment time:
            t += ts
        tx_queue.put(("C1", samples))
        tx_queue.put(("C2", samples2))
        time.sleep(dt)
    print("rip thread")


class DemoGraphWidget(QWidget):
    def __init__(self):
        super().__init__()

        self.series1 = ZoomSerie()
        self.series2 = ZoomSerie()
        self._serie_map = {
            "C1": self.series1,
            "C2": self.series2,
        }

        self._chart_widget = ChartWidget()
        self._chart_widget.chart.add_serie(self.series1)
        self._chart_widget.chart.add_serie(self.series2)

        l = QVBoxLayout()
        l.addWidget(self._chart_widget)
        self.setLayout(l)

        self.resize(600, 400)

        self._rx_queue = queue.Queue()
        self._timer = QTimer()
        self._timer.timeout.connect(self.on_timeout)
        self._timer.start(50)

    def on_timeout(self):
        if not self._rx_queue.empty():
            while not self._rx_queue.empty():
                chunk = self._rx_queue.get()
                name, samples = chunk
                serie = self._serie_map[name]
                serie.add_samples(samples)
                self._rx_queue.task_done()
            self.update()


if __name__ == "__main__":
    main()